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

    let args_path = std::env::temp_dir().join(format!(
        "shipping-oracle-sdk-{}-consume_args.json",
        std::process::id()
    ));
    tx_args
        .write_to_path(&args_path)
        .expect("args json should be written");
    let written = std::fs::read_to_string(&args_path).expect("args json should be readable");
    assert!(written.contains("p_tracking_number_hash"));
    assert!(written.contains(&tx_args.p_status));
    let _ = std::fs::remove_file(args_path);

    let release_args = commitment.to_release_escrow_args_json(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa#0",
    );
    assert_eq!(release_args.p_status, hex::encode("DELIVERED"));
    assert_eq!(release_args.p_signature, commitment.attestation.signature);
    assert_eq!(
        release_args.escrow_utxo,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa#0"
    );

    let lock_args = commitment.to_lock_escrow_ada_args_json(
        10_000_000,
        "11111111111111111111111111111111111111111111111111111111",
        "22222222222222222222222222222222222222222222222222222222",
        "order-123",
        1_712_000_000,
        1_713_000_000,
    );
    assert_eq!(lock_args.quantity, 10_000_000);
    assert_eq!(lock_args.order_id, hex::encode("order-123"));
    assert_eq!(
        lock_args.carrier_hash,
        commitment.attestation.data.carrier_hash
    );
    assert_eq!(
        lock_args.tracking_number_hash,
        commitment.attestation.data.tracking_number_hash
    );

    let refund_args = commitment.to_refund_escrow_args_json(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa#0",
    );
    assert_eq!(
        refund_args.escrow_utxo,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa#0"
    );
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
