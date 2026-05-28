mod common;

use shipping_oracle_sdk::{OracleClient, OracleStatus, ShipmentReference};

use common::{
    DELIVERED_TRACKING, SHIPPO_CARRIER, TEST_PUBLIC_KEY_HEX, start_mocked_oracle,
    stub_shippo_status,
};

#[tokio::test]
async fn health_endpoint_responds() {
    let (_shippo, oracle) = start_mocked_oracle().await;
    let client = OracleClient::new(&oracle.base_url);

    let response = client.health().await.expect("health response");
    assert_eq!(response.status, "ok");
}

#[tokio::test]
async fn prepare_commitment_links_context_and_tx3_args() {
    let (shippo, oracle) = start_mocked_oracle().await;
    stub_shippo_status(&shippo, DELIVERED_TRACKING, "DELIVERED").await;

    let client = OracleClient::new(&oracle.base_url)
        .with_expected_public_key_hex(TEST_PUBLIC_KEY_HEX)
        .expect("expected public key");

    let commitment = client
        .prepare_commitment("order-123".to_string(), SHIPPO_CARRIER, DELIVERED_TRACKING)
        .await
        .expect("prepared commitment");

    assert_eq!(commitment.context, "order-123");
    assert_eq!(commitment.attestation.data.status, OracleStatus::Delivered);
    commitment.verify().expect("verified commitment");

    let tx_args = commitment
        .to_cli_args_json()
        .expect("tx3 cli args should be available");
    assert_eq!(tx_args.p_status, hex::encode("DELIVERED"));
    assert_eq!(tx_args.p_signature, commitment.attestation.signature);
}

#[tokio::test]
async fn fetch_for_uses_typed_reference() {
    let (shippo, oracle) = start_mocked_oracle().await;
    stub_shippo_status(&shippo, DELIVERED_TRACKING, "DELIVERED").await;

    let client = OracleClient::new(&oracle.base_url);
    let shipment = ShipmentReference::new(SHIPPO_CARRIER, DELIVERED_TRACKING);
    let attestation = client.fetch_for(&shipment).await.expect("attestation");

    assert_eq!(attestation.shipment_reference(), shipment);
    attestation.verify().expect("valid attestation");
}
