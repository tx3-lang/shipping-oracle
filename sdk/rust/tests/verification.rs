mod common;

use shipping_oracle_sdk::{OracleClient, OracleSdkError};

use common::{
    DELIVERED_TRACKING, SHIPPO_CARRIER, TEST_PUBLIC_KEY_HEX, start_mocked_oracle,
    stub_shippo_status,
};

#[tokio::test]
async fn verify_rejects_wrong_expected_key() {
    let (shippo, oracle) = start_mocked_oracle().await;
    stub_shippo_status(&shippo, DELIVERED_TRACKING, "DELIVERED").await;
    let client = OracleClient::new(&oracle.base_url);
    let attestation = client
        .fetch_attestation(SHIPPO_CARRIER, DELIVERED_TRACKING)
        .await
        .expect("attestation");

    let err = attestation
        .verify_with_expected_public_key_hex(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        )
        .expect_err("wrong key must fail");

    assert!(matches!(err, OracleSdkError::UnexpectedPublicKey));
}

#[tokio::test]
async fn verify_detects_tampered_cbor() {
    let (shippo, oracle) = start_mocked_oracle().await;
    stub_shippo_status(&shippo, DELIVERED_TRACKING, "DELIVERED").await;
    let client = OracleClient::new(&oracle.base_url)
        .with_expected_public_key_hex(TEST_PUBLIC_KEY_HEX)
        .expect("expected public key");
    let mut attestation = client
        .fetch_attestation(SHIPPO_CARRIER, DELIVERED_TRACKING)
        .await
        .expect("attestation");

    attestation.cbor_hex.push_str("00");

    let err = attestation.verify().expect_err("tampered cbor must fail");
    assert!(matches!(
        err,
        OracleSdkError::InvalidSignature | OracleSdkError::CborMismatch
    ));
}

#[tokio::test]
async fn fetch_attestation_surfaces_upstream_errors() {
    let (shippo, oracle) = start_mocked_oracle().await;
    stub_shippo_status(&shippo, DELIVERED_TRACKING, "DELIVERED").await;
    let client = OracleClient::new(&oracle.base_url);

    let err = client
        .prepare_commitment("ctx", SHIPPO_CARRIER, "DIFFERENT_TRACKING")
        .await
        .expect_err("upstream mismatch should fail as api error");

    assert!(matches!(err, OracleSdkError::Api { status: 502, .. }));
}
