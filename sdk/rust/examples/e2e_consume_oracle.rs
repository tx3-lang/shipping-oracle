use std::path::PathBuf;

use shipping_oracle_sdk::{OracleClient, ShipmentReference};

#[derive(Debug)]
struct OrderContext {
    order_id: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let base_url =
        std::env::var("ORACLE_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:3000".to_string());
    let carrier = std::env::var("SHIPMENT_CARRIER").unwrap_or_else(|_| "shippo".to_string());
    let tracking_number = std::env::var("SHIPMENT_TRACKING_NUMBER")
        .unwrap_or_else(|_| "SHIPPO_DELIVERED".to_string());
    let order_id = std::env::var("ORDER_ID").unwrap_or_else(|_| "ord_demo_123".to_string());
    let args_out = std::env::var("TX3_ARGS_OUT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp/consume_args.json"));
    let trix_profile = std::env::var("TRIX_PROFILE").unwrap_or_else(|_| "local".to_string());

    let client = match std::env::var("ORACLE_PUBLIC_KEY") {
        Ok(public_key) => OracleClient::new(&base_url).with_expected_public_key_hex(&public_key)?,
        Err(_) => OracleClient::new(&base_url),
    };

    let shipment = ShipmentReference::new(&carrier, &tracking_number);
    let commitment = client
        .prepare_for(
            OrderContext {
                order_id: order_id.clone(),
            },
            &shipment,
        )
        .await?;

    let args_json = commitment.to_cli_args_json()?;
    args_json.write_to_path(&args_out)?;

    println!("Prepared Shipping Oracle consumer transaction inputs");
    println!("order_id: {}", commitment.context.order_id);
    println!("carrier: {}", commitment.attestation.plaintext.carrier);
    println!(
        "tracking_number: {}",
        commitment.attestation.plaintext.tracking_number
    );
    println!("status: {}", commitment.attestation.data.status);
    println!("timestamp: {}", commitment.attestation.data.timestamp);
    println!("args_json: {}", args_out.display());
    println!();
    println!("Next step:");
    println!(
        "  cd tx3 && trix invoke -p {} --args-json-path \"{}\"",
        trix_profile,
        args_out.display()
    );

    Ok(())
}
