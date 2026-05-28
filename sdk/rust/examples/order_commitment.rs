use shipping_oracle_sdk::{OracleClient, ShipmentReference};

#[derive(Debug)]
struct OrderContext {
    order_id: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let base_url =
        std::env::var("ORACLE_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:3000".to_string());
    let shipment = ShipmentReference::new("shippo", "SHIPPO_DELIVERED");
    let client = match std::env::var("ORACLE_PUBLIC_KEY") {
        Ok(public_key) => OracleClient::new(&base_url).with_expected_public_key_hex(&public_key)?,
        Err(_) => OracleClient::new(&base_url),
    };

    let commitment = client
        .prepare_for(
            OrderContext {
                order_id: "ord_123".to_string(),
            },
            &shipment,
        )
        .await?;

    let tx_args = commitment.to_cli_args_json()?;
    println!("linked order: {}", commitment.context.order_id);
    println!("shipment status: {}", commitment.attestation.data.status);
    println!("tx3 args:\n{}", tx_args.as_json_string());

    Ok(())
}
