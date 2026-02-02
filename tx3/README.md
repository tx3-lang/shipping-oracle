# Shipping Oracle - Tx3

## Overview
The tx3 protocol defines the Cardano transactions used by the Shipping Oracle. It specifies the parties, datums, and flows for publishing the validator, tracking a shipment, and closing a shipment with a final status.

## Modules and Services
- `main.tx3`: tx3 protocol definition including parties, datums, and transactions.
- `trix.toml`: Protocol metadata and codegen configuration.

## Transactions
1. **publish**: The oracle publishes the validator script on-chain using `VALIDATOR_SCRIPT_BYTES`.
2. **track_shipment**: A customer funds a tracking UTxO with `TrackingDatum` (carrier, tracking number, outbox address).
3. **close_shipment**: The oracle consumes the tracking UTxO and produces a `ShipmentDatum` output plus a payment output.

## Environment and Config
Env values are required by the tx3 environment and are provided via `.env.preview` (or another profile).

- `VALIDATOR_SCRIPT_BYTES`: Compiled validator script bytes.
- `VALIDATOR_SCRIPT_REF`: Reference script UTxO (`TxHash#TxIx`).
- `ORACLE_PKH`: Oracle public key hash.
