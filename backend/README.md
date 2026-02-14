# Shipping Oracle - Data Fetcher

The data fetcher component for the Shipping Oracle.

## Overview
This component watches the Cardano blockchain for [Tracking UTxOs](../README.md#track-shipment) in the oracle address.<br />
If a tracking UTxO is found, it queries the shipment status using the Shippo API.<br />
If the shipment status qualifies as final (`DELIVERED`/`NOT_DELIVERED`) a [close shipment transaction](../README.md#close-shipment) is submitted to the outbox adress.<br />
This will close the tracking request registering the shipment status and collecting the funds to the oracle.

## Modules and Services
- `config`: Loads runtime configuration from environment variables.
- `scheduler`: Runs the cron-driven execution loop and triggers fetch jobs.
- `fetcher`: Orchestrates the end-to-end shipment update workflow.
- `blockchain`: `CardanoClient` queries Blockfrost for tracking UTxOs and submit the shipment updates.
- `shipment`: `ShipmentClient` calls Shippo to fetch shipments tracking statuses.
- `models`: Shared data structures for tracking responses and datum parsing.
- `tx3`: Client wrapper for resolving transactions via the TRP service.

## Data Flow
1. `scheduler` triggers a fetch job based on `CRON_SCHEDULE`.
2. `fetcher` asks `blockchain` to search Tracking UTxOs in the Oracle address using the Blockfrost API.
3. For each tracking UTxO, `shipment` retrieves status from the Shippo API.
4. `fetcher` decides whether the shipment status is final.
5. If final, `blockchain` uses `tx3` to resolve a close-shipment transaction and submits it via Blockfrost API.

## Setup and Run

### Prerequisites
- Rust 1.70+ (install via [rustup](https://rustup.rs/))
- Access to:
  - [Shippo API](https://docs.goshippo.com/) (for tracking data)
  - [Blockfrost API](https://docs.blockfrost.io/) (for Cardano queries)
  - [TRP service](https://docs.txpipe.io/tx3) (for tx3 resolution)

### Configuration
1. Copy the example environment file:
   ```bash
   cp .env.example .env
   ```

2. Edit `.env` and set required values.

### Build
```bash
cargo build --release
```

### Run
```bash
cargo run --release
```

## Environment Variables
All configuration is loaded from environment variables (see `.env.example`).

- `CRON_SCHEDULE`: Cron expression for the scheduler (default: `0 */5 * * * *`).
- `SHIPPO_API_KEY`: Shippo API key for tracking lookups.
- `VALIDATOR_SCRIPT_REF`: Reference script UTxO (`TxHash#TxIx`).
- `ORACLE_SK`: Oracle signing key (hex).
- `ORACLE_PKH`: Oracle public key hash (hex).
- `ORACLE_ADDRESS`: Cardano Oracle address holding tracking UTxOs.
- `ORACLE_PAYMENT_ADDRESS`: Address to receive Oracle transaction funds.
- `BLOCKFROST_URL`: Blockfrost authenticated API url.
- `TRP_URL`: TRP endpoint used by the tx3 client.
- `TRP_API_KEY`: API key for the TRP endpoint (default: empty).
