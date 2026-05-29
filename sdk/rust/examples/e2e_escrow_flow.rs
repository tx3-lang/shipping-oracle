use std::path::PathBuf;
use std::{io, str::FromStr};

use shipping_oracle_sdk::{OracleClient, ShipmentReference, refund_escrow_args_json};

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
    let order_id = std::env::var("ORDER_ID").unwrap_or_else(|_| "ord_demo_escrow_123".to_string());
    let quantity = parse_i64_env("ESCROW_LOVELACE", 10_000_000)?;
    let buyer_pkh = required_hex_env("BUYER_PKH", 28)?;
    let merchant_pkh = required_hex_env("MERCHANT_PKH", 28)?;
    let lock_args_out = path_env("LOCK_ESCROW_ARGS_OUT", "/tmp/lock_escrow_ada_args.json");
    let release_args_out = path_env("RELEASE_ESCROW_ARGS_OUT", "/tmp/release_escrow_args.json");
    let refund_args_out = path_env("REFUND_ESCROW_ARGS_OUT", "/tmp/refund_escrow_args.json");
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

    let paid_at = parse_i64_env("PAID_AT", commitment.attestation.data.timestamp)?;
    let refund_after = parse_i64_env("REFUND_AFTER", paid_at + 7 * 24 * 60 * 60)?;

    let lock_args = commitment.to_lock_escrow_ada_args_json(
        quantity,
        buyer_pkh,
        merchant_pkh,
        &commitment.context.order_id,
        paid_at,
        refund_after,
    );
    lock_args.write_to_path(&lock_args_out)?;

    println!("Prepared Shipping Oracle escrow transaction inputs");
    println!("order_id: {}", commitment.context.order_id);
    println!("carrier: {}", commitment.attestation.plaintext.carrier);
    println!(
        "tracking_number: {}",
        commitment.attestation.plaintext.tracking_number
    );
    println!("status: {}", commitment.attestation.data.status);
    println!("timestamp: {}", commitment.attestation.data.timestamp);
    println!("quantity_lovelace: {quantity}");
    println!("paid_at: {paid_at}");
    println!("refund_after: {refund_after}");
    println!("lock_args_json: {}", lock_args_out.display());
    println!();
    println!("Lock escrow:");
    println!(
        "  cd tx3 && trix invoke -p {} --args-json-path \"{}\"",
        trix_profile,
        lock_args_out.display()
    );

    match std::env::var("ESCROW_UTXO") {
        Ok(escrow_utxo) => {
            let release_args = commitment.to_release_escrow_args_json(&escrow_utxo);
            release_args.write_to_path(&release_args_out)?;

            let refund_args = refund_escrow_args_json(&escrow_utxo);
            refund_args.write_to_path(&refund_args_out)?;

            println!();
            println!("Release escrow after DELIVERED attestation:");
            println!(
                "  cd tx3 && trix invoke -p {} --args-json-path \"{}\"",
                trix_profile,
                release_args_out.display()
            );
            println!();
            println!("Refund escrow after timeout:");
            println!(
                "  cd tx3 && trix invoke -p {} --args-json-path \"{}\"",
                trix_profile,
                refund_args_out.display()
            );
            println!();
            println!("release_args_json: {}", release_args_out.display());
            println!("refund_args_json: {}", refund_args_out.display());
        }
        Err(_) => {
            println!();
            println!(
                "Set ESCROW_UTXO=<lock_tx_hash>#<output_index> and rerun this example to write release/refund args."
            );
        }
    }

    Ok(())
}

fn path_env(name: &str, default: &str) -> PathBuf {
    std::env::var(name)
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(default))
}

fn parse_i64_env(name: &str, default: i64) -> Result<i64, Box<dyn std::error::Error>> {
    Ok(match std::env::var(name) {
        Ok(value) => i64::from_str(&value)?,
        Err(_) => default,
    })
}

fn required_hex_env(
    name: &'static str,
    expected_bytes: usize,
) -> Result<String, Box<dyn std::error::Error>> {
    let value = std::env::var(name).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("missing required env var {name}"),
        )
    })?;
    let bytes = hex::decode(&value)?;
    if bytes.len() != expected_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "{name} must be {expected_bytes} bytes hex, got {} bytes",
                bytes.len()
            ),
        )
        .into());
    }
    Ok(value)
}
