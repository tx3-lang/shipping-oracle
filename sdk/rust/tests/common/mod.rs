use std::sync::Arc;

use serde_json::json;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use shipping_oracle::api;
use shipping_oracle::config::Config;
use shipping_oracle::oracle_service::{OracleService, load_signing_key};
use shipping_oracle::shipment::ShipmentClient;

pub const TEST_SK_HEX: &str = "0101010101010101010101010101010101010101010101010101010101010101";
pub const TEST_PUBLIC_KEY_HEX: &str =
    "8a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c";
pub const SHIPPO_CARRIER: &str = "shippo";
pub const DELIVERED_TRACKING: &str = "SHIPPO_DELIVERED";
pub const TRANSIT_TRACKING: &str = "SHIPPO_TRANSIT";
pub const PRE_TRANSIT_TRACKING: &str = "SHIPPO_PRE_TRANSIT";
pub const RETURNED_TRACKING: &str = "SHIPPO_RETURNED";
pub const UNKNOWN_TRACKING: &str = "SHIPPO_UNKNOWN";
pub const ERROR_TRACKING: &str = "SHIPPO_ERROR";
pub const FROZEN_TIMESTAMP: i64 = 1712000000;

pub struct TestServer {
    pub base_url: String,
    _handle: JoinHandle<()>,
}

pub async fn start_oracle(shippo_base_url: String) -> TestServer {
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

pub async fn start_mocked_oracle() -> (MockServer, TestServer) {
    let shippo = MockServer::start().await;
    let oracle = start_oracle(shippo.uri()).await;
    (shippo, oracle)
}

pub async fn stub_shippo_status(server: &MockServer, tracking_number: &str, status: &str) {
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

pub async fn stub_shippo_error(server: &MockServer, tracking_number: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/tracks/{SHIPPO_CARRIER}/{tracking_number}")))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(server)
        .await;
}
