# Shipping Oracle Rust SDK

Rust SDK for applications that need to consume the Shipping Oracle over HTTP, verify the returned attestation, and convert it into tx3-ready arguments for the on-chain `consume_oracle_data` flow.

## What It Covers

- Fetch `GET /v1/shipment` attestations from the oracle backend.
- Verify Ed25519 signatures over the canonical CBOR payload.
- Check that the signed `OracleData` matches the returned plaintext shipment identifiers.
- Convert the attestation into arguments for `tx3/main.tx3::consume_oracle_data`.
- Keep application context such as `order_id` or event ids attached to the resulting commitment package.

This SDK is the repo's first Milestone 3 deliverable for acceptance criterion `A1`: developers can link application events to on-chain shipment tracking commitments without hand-assembling `curl`, `jq`, and hex-conversion steps.

## Install

Add the crate from this repository path while the SDK is still in-repo:

```toml
[dependencies]
shipping-oracle-sdk = { path = "../sdk/rust" }
```

## Quick Start

```rust
use shipping_oracle_sdk::OracleClient;

#[derive(Debug)]
struct OrderContext {
    order_id: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = OracleClient::new("http://127.0.0.1:3000");

    let commitment = client
        .prepare_commitment(
            OrderContext {
                order_id: "ord_123".to_string(),
            },
            "shippo",
            "SHIPPO_DELIVERED",
        )
        .await?;

    commitment.verify()?;

    let tx_args = commitment.to_cli_args_json()?;
    println!("linked order: {}", commitment.context.order_id);
    println!("tx args:\n{}", tx_args.as_json_string());

    Ok(())
}
```

## Public API

### `OracleClient`

- `OracleClient::new(base_url)`
- `with_expected_public_key_hex(public_key_hex)`
- `health()`
- `fetch_attestation(carrier, tracking_number)`
- `prepare_commitment(context, carrier, tracking_number)`

If you configure `with_expected_public_key_hex`, `prepare_commitment` verifies that the response was signed by the expected oracle key before returning it.

### `OracleAttestation`

Mirrors the HTTP response:

```json
{
  "data": {
    "carrier_hash": "...",
    "tracking_number_hash": "...",
    "status": "DELIVERED",
    "timestamp": 1712000000
  },
  "plaintext": {
    "carrier": "shippo",
    "tracking_number": "SHIPPO_DELIVERED"
  },
  "signature": "...",
  "public_key": "...",
  "cbor_hex": "..."
}
```

Important details:

- `signature` is over the raw bytes in `cbor_hex`.
- `cbor_hex` must not be reserialized or normalized.
- `status` is an `OracleStatus` enum in the SDK, but converts back to the exact on-chain byte vocabulary.

### `PreparedCommitment<TContext>`

Holds:

- your application context
- the verified oracle attestation
- helper conversion methods for the consumer transaction

Supported conversions:

- `to_consume_oracle_data_params()` for programmatic tx3 integration
- `to_cli_args_json()` for `trix invoke --args-json-path ...`

The CLI args JSON looks like:

```json
{
  "p_carrier_hash": "...",
  "p_tracking_number_hash": "...",
  "p_status": "44454c495645524544",
  "p_timestamp": 1712000000,
  "p_signature": "..."
}
```

That matches the manual flow documented in `TESTING.md`, but the SDK generates it directly.

## Verification Behavior

`verify()` checks:

- the signature validates under `public_key`
- the canonical CBOR in `cbor_hex` matches the declared `data` fields
- `plaintext.carrier` hashes to `data.carrier_hash`
- `plaintext.tracking_number` hashes to `data.tracking_number_hash`

`prepare_commitment()` also checks that the response plaintext matches the request you made, so an application cannot accidentally link the wrong shipment to an order.

## Examples

Run the examples against a local backend:

```bash
cd sdk/rust
cargo run --example raw_attestation
cargo run --example order_commitment
```

Optional environment variables:

- `ORACLE_BASE_URL` defaults to `http://127.0.0.1:3000`
- `ORACLE_PUBLIC_KEY` pins the expected oracle verification key

## Tests And Evidence

```bash
cd sdk/rust
cargo test --all-targets -- --nocapture
```

The SDK test suite:

- spins up the existing backend server logic with Shippo stubbed via `wiremock`
- verifies signature and CBOR integrity
- checks tx3 argument generation
- writes milestone evidence reports to `sdk/rust/reports/`

Artifacts:

- `sdk-integration.json`
- `sdk-integration.md`

These are uploaded by `.github/workflows/sdk.yml` and serve as SDK-specific milestone evidence alongside the source code and docs.
