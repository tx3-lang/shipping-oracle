use std::sync::Arc;

use ed25519_dalek::{Signature, Verifier};
use pallas::codec::minicbor;
use serde_json::json;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use shipping_oracle::api;
use shipping_oracle::config::Config;
use shipping_oracle::models::SignedOracleResponse;
use shipping_oracle::oracle_service::{
    OracleService, blake2b256, load_signing_key, plutus_oracle_data,
};
use shipping_oracle::shipment::ShipmentClient;

const TEST_SK_HEX: &str = "0101010101010101010101010101010101010101010101010101010101010101";
const SHIPPO_CARRIER: &str = "shippo";
const DELIVERED_TRACKING: &str = "SHIPPO_DELIVERED";
const TRANSIT_TRACKING: &str = "SHIPPO_TRANSIT";
const UNKNOWN_TRACKING: &str = "SHIPPO_UNKNOWN";
const FROZEN_TIMESTAMP: i64 = 1712000000;

struct TestServer {
    base_url: String,
    _handle: JoinHandle<()>,
}

async fn start_oracle(shippo_base_url: String) -> TestServer {
    let config = Config {
        listen_address: "127.0.0.1:0".to_string(),
        shippo_api_key: "test-token".to_string(),
        oracle_sk: TEST_SK_HEX.to_string(),
        oracle_pkh: "00".repeat(28),
        oracle_address: "addr_test1placeholder".to_string(),
        trp_url: "http://localhost:8000".to_string(),
        trp_api_key: None,
    };
    let shipment_client =
        ShipmentClient::with_base_url(&config, shippo_base_url).expect("shipment client");
    let signing_key = load_signing_key(&config.oracle_sk).expect("load sk");
    let service = Arc::new(OracleService::with_clock(
        shipment_client,
        signing_key,
        Box::new(|| FROZEN_TIMESTAMP),
    ));

    let listener = TcpListener::bind(&config.listen_address)
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("local addr");
    let app = api::create_router(service);
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.expect("oracle server");
    });

    TestServer {
        base_url: format!("http://{addr}"),
        _handle: handle,
    }
}

async fn stub_shippo_status(server: &MockServer, tracking_number: &str, status: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/tracks/{SHIPPO_CARRIER}/{tracking_number}")))
        .and(header("Authorization", "ShippoToken test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "carrier": SHIPPO_CARRIER,
            "tracking_number": tracking_number,
            "tracking_status": {
                "status": status,
                "status_details": "stub"
            }
        })))
        .mount(server)
        .await;
}

fn assert_signature_round_trip(response: &SignedOracleResponse) {
    let vk_bytes = hex::decode(&response.public_key).expect("vk hex");
    let vk: [u8; 32] = vk_bytes.try_into().expect("32-byte vk");
    let vk = ed25519_dalek::VerifyingKey::from_bytes(&vk).expect("vk decode");

    let sig_bytes = hex::decode(&response.signature).expect("sig hex");
    let sig_bytes: [u8; 64] = sig_bytes.try_into().expect("64-byte sig");
    let signature = Signature::from_bytes(&sig_bytes);

    let cbor_bytes = hex::decode(&response.cbor_hex).expect("cbor hex");
    vk.verify(&cbor_bytes, &signature)
        .expect("signature must verify");
}

fn assert_cbor_matches_plaintext(response: &SignedOracleResponse) {
    let carrier_hash = blake2b256(response.plaintext.carrier.as_bytes());
    let tracking_hash = blake2b256(response.plaintext.tracking_number.as_bytes());
    assert_eq!(hex::encode(&carrier_hash), response.data.carrier_hash);
    assert_eq!(
        hex::encode(&tracking_hash),
        response.data.tracking_number_hash
    );

    let plutus = plutus_oracle_data(
        carrier_hash,
        tracking_hash,
        response.data.status.as_bytes().to_vec(),
        response.data.timestamp,
    );
    let expected = hex::encode(minicbor::to_vec(&plutus).expect("cbor"));
    assert_eq!(expected, response.cbor_hex);
}

#[tokio::test]
async fn health_endpoint_responds() {
    let shippo = MockServer::start().await;
    let server = start_oracle(shippo.uri()).await;

    let response = reqwest::get(format!("{}/health", server.base_url))
        .await
        .expect("GET /health");
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn shipment_endpoint_returns_signed_delivered_status() {
    let shippo = MockServer::start().await;
    stub_shippo_status(&shippo, DELIVERED_TRACKING, "DELIVERED").await;
    let server = start_oracle(shippo.uri()).await;

    let response = reqwest::get(format!(
        "{}/v1/shipment?carrier={SHIPPO_CARRIER}&tracking_number={DELIVERED_TRACKING}",
        server.base_url
    ))
    .await
    .expect("GET /v1/shipment");
    assert_eq!(response.status(), 200);

    let body: SignedOracleResponse = response.json().await.expect("response body");
    assert_eq!(body.data.status, "DELIVERED");
    assert_eq!(body.data.timestamp, FROZEN_TIMESTAMP);
    assert_eq!(body.plaintext.carrier, SHIPPO_CARRIER);
    assert_eq!(body.plaintext.tracking_number, DELIVERED_TRACKING);
    assert_cbor_matches_plaintext(&body);
    assert_signature_round_trip(&body);
}

#[tokio::test]
async fn shipment_endpoint_maps_transit_and_unknown_statuses() {
    let shippo = MockServer::start().await;
    stub_shippo_status(&shippo, TRANSIT_TRACKING, "TRANSIT").await;
    stub_shippo_status(&shippo, UNKNOWN_TRACKING, "WEIRD_STATUS").await;
    let server = start_oracle(shippo.uri()).await;

    let transit: SignedOracleResponse = reqwest::get(format!(
        "{}/v1/shipment?carrier={SHIPPO_CARRIER}&tracking_number={TRANSIT_TRACKING}",
        server.base_url
    ))
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
    assert_eq!(transit.data.status, "IN_TRANSIT");
    assert_cbor_matches_plaintext(&transit);
    assert_signature_round_trip(&transit);

    let unknown: SignedOracleResponse = reqwest::get(format!(
        "{}/v1/shipment?carrier={SHIPPO_CARRIER}&tracking_number={UNKNOWN_TRACKING}",
        server.base_url
    ))
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
    assert_eq!(unknown.data.status, "UNKNOWN");
    assert_signature_round_trip(&unknown);
}

#[tokio::test]
async fn shipment_endpoint_surfaces_upstream_errors_as_bad_gateway() {
    let shippo = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!(
            "/tracks/{SHIPPO_CARRIER}/{DELIVERED_TRACKING}"
        )))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(&shippo)
        .await;
    let server = start_oracle(shippo.uri()).await;

    let response = reqwest::get(format!(
        "{}/v1/shipment?carrier={SHIPPO_CARRIER}&tracking_number={DELIVERED_TRACKING}",
        server.base_url
    ))
    .await
    .unwrap();
    assert_eq!(response.status(), 502);
}
