# Shipping Oracle — Backend

Pull-based HTTP oracle. Receives shipment queries, fetches status from the carrier API (Shippo), and returns a signed attestation that consumers embed in their own Cardano transactions. The backend never submits transactions itself.

## Overview

```
HTTP consumer ──► GET /v1/shipment?carrier=...&tracking_number=...
                          │
                          ▼
            ┌────────────────────────────┐
            │        OracleService       │
            │                            │
            │  1. fetch Shippo status    │
            │  2. normalise status       │
            │  3. blake2b hash carrier/  │
            │     tracking number        │
            │  4. build PlutusData       │
            │  5. minicbor serialize     │
            │  6. Ed25519 sign(CBOR)     │
            └────────────────────────────┘
                          │
                          ▼
                  SignedOracleResponse
              (data + plaintext + signature
               + public_key + cbor_hex)
```

The `cbor_hex` bytes are the exact message that was signed. Consumers must embed them verbatim in the on-chain redeemer — re-serialising the structure would risk a mismatch with `builtin.serialise_data` on-chain. Byte-level alignment is pinned by `tests/cbor_alignment.rs` and the matching Aiken tests under `onchain/lib/cbor_alignment_tests.ak`.

## Modules

- `config` — environment-variable configuration.
- `api` — axum HTTP router (`/v1/shipment`, `/health`).
- `oracle_service` — signing logic (hash, PlutusData, minicbor, Ed25519).
- `shipment` — Shippo API client + status normalisation to the 5 on-chain statuses.
- `models` — request/response types + Shippo DTOs.
- `tx3` — auto-generated tx3 bindings (with documented post-patches; see the file header).

## Setup

Prereqs: Rust 1.70+, Shippo API access, a running TRP endpoint (for tx3 consumers).

```bash
cp .env.example .env     # fill SHIPPO_API_KEY, ORACLE_SK, ORACLE_PKH, ORACLE_ADDRESS, TRP_URL
cargo run                # starts the HTTP server on LISTEN_ADDRESS (default 0.0.0.0:3000)
```

## Environment Variables

| Variable         | Required | Default           | Purpose                                                      |
| ---------------- | -------- | ----------------- | ------------------------------------------------------------ |
| `SHIPPO_API_KEY` | ✓        |                   | Shippo tracking API token                                    |
| `ORACLE_SK`      | ✓        |                   | Ed25519 signing key (32 bytes, hex)                          |
| `ORACLE_PKH`     | ✓        |                   | Verification-key hash (28 bytes, hex)                        |
| `ORACLE_ADDRESS` | ✓        |                   | Cardano address the oracle controls                          |
| `TRP_URL`        | ✓        |                   | TRP endpoint used by consumers to resolve tx3 txs            |
| `LISTEN_ADDRESS` |          | `0.0.0.0:3000`    | HTTP bind address                                            |
| `TRP_API_KEY`    |          |                   | Required when the TRP endpoint enforces authentication       |

## Tests

```bash
cargo test                 # all
cargo test --test cbor_alignment      # PlutusData CBOR byte-level check vs Aiken
cargo test --test signature_vectors   # Deterministic Ed25519 vectors shared with Aiken
cargo test --test integration         # HTTP integration (Shippo stubbed via wiremock)
```

No external network or secrets are required to run the test suite.

## License

Licensed under the Apache License, Version 2.0. See `LICENSE`.
