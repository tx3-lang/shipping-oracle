use shipping_oracle_sdk::OracleClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let base_url =
        std::env::var("ORACLE_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:3000".to_string());
    let client = OracleClient::new(base_url);
    let attestation = client
        .fetch_attestation("shippo", "SHIPPO_DELIVERED")
        .await?;

    attestation.verify()?;

    println!("status: {}", attestation.data.status);
    println!("cbor_hex: {}", attestation.cbor_hex);

    Ok(())
}
