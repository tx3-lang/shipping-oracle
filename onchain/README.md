# Shipping Oracle — On-chain

Aiken validators that back the **pull-based** Shipping Oracle. The on-chain side never receives shipment data directly; it verifies an Ed25519 signature produced off-chain by the oracle backend against a verification key stored in a static governance UTxO identified by a one-shot NFT.

## Overview

```
   ┌─────────────────────────┐
   │   governance_nft.ak     │   one-shot minting policy
   │                         │   • requires consuming a hardcoded seed UTxO
   │   purpose: mint         │   • allows minting exactly one token
   └───────────┬─────────────┘
               │  (NFT lives in)
               ▼
   ┌─────────────────────────┐
   │   Governance UTxO       │   inline datum = GovernanceDatum { oracle_vk }
   │   (at oracle address)   │   carries qty=1 of (gov_policy_id, "GOV")
   └───────────┬─────────────┘
               │  (referenced by)
               ▼
   ┌─────────────────────────┐
   │   oracle.ak             │   withdrawal validator (Pyth-style)
   │                         │   1. find ref input carrying the gov NFT
   │   purpose: withdraw     │   2. read oracle_vk from its datum
   │                         │   3. verify_ed25519(oracle_vk,
   │                         │        serialise_data(redeemer.data),
   │                         │        redeemer.signature)
   └─────────────────────────┘
```

The validator runs as `withdraw` so consumers attach it via a 0-lovelace withdrawal from the script's reward address (the "withdrawal trick"). They never need to spend a UTxO governed by the oracle.

## Modules

- `validators/governance_nft.ak` — one-shot minting policy. Guarantees the governance NFT is unforgeable by requiring consumption of `seed_utxo_*`.
- `validators/oracle.ak` — withdrawal validator. Verifies the Ed25519 signature carried in `OracleRedeemer` against the `oracle_vk` stored in the governance UTxO datum.
- `lib/types.ak` — shared types (`GovernanceDatum`, `OracleData`, `OracleRedeemer`) and the on-chain status vocabulary (`DELIVERED`, `NOT_DELIVERED`, `IN_TRANSIT`, `PRE_TRANSIT`, `UNKNOWN`).
- `lib/cbor_alignment_tests.ak` — pinned CBOR vectors that must stay byte-identical to `backend/tests/cbor_alignment.rs`. Locks in `builtin.serialise_data(OracleData) == minicbor::to_vec(PlutusData)`.
- `aiken.toml` — project metadata + the `[config.default]` block consumed by both validators.

## Configuration (`aiken.toml [config.default]`)

| Parameter | Used by | Meaning |
|---|---|---|
| `seed_utxo_tx_hash`, `seed_utxo_index` | `governance_nft.ak` | UTxO that the one-shot mint must consume. Replace with a real UTxO controlled by the oracle wallet before deploy. |
| `governance_asset_name` | `governance_nft.ak`, `oracle.ak` | Asset name of the governance NFT (hex). Default `474f56` = ASCII `"GOV"`. |
| `gov_policy_id` | `oracle.ak` | Policy id (28-byte script hash) of `governance_nft`. Filled on the **second pass** of `aiken build` from the compiled mint policy hash. |

> Bytes values must use the object form `{ bytes = "...", encoding = "base16" }`. Omitting `encoding` makes `aiken check` fail with `missing field encoding`.

## Build (two-pass)

`oracle.ak` references `gov_policy_id`, which is the hash of `governance_nft`. That hash is only known after the mint is compiled, so two passes are required:

```bash
cd onchain

# Pass 1 — compile with whatever placeholder is in aiken.toml
aiken build

# Read the real policy id from the compiled mint
jq -r '.validators[] | select(.title=="governance_nft.governance_nft.mint") | .hash' plutus.json

# Paste it into aiken.toml under [config.default].gov_policy_id
#   gov_policy_id = { bytes = "<hash>", encoding = "base16" }

# Pass 2 — recompile so oracle.ak is bound to the real policy id
aiken build
```

If you ever change `seed_utxo_tx_hash` / `seed_utxo_index`, the mint script bytes change → the policy id changes → you **must** rerun the two passes. Otherwise the live `oracle.ak` will validate against a stale policy id and reject every withdrawal.

## Tests

```bash
aiken check                    # all modules
aiken check -m oracle          # withdrawal validator
aiken check -m governance_nft  # mint policy
```

Coverage:

- `oracle.ak`: `withdraw_valid_signature`, `withdraw_invalid_signature`, `withdraw_tampered_data`, `withdraw_missing_governance_nft`.
- `governance_nft.ak`: `mint_valid`, `mint_missing_seed_input`, `mint_wrong_asset_name`, `mint_wrong_quantity`, `mint_extra_asset`.
- `cbor_alignment_tests.ak`: three pinned CBOR vectors checked against the matching Rust tests in `backend/tests/cbor_alignment.rs`. Update both files together.

The Ed25519 vectors used by `withdraw_valid_signature` are produced by `backend/tests/signature_vectors.rs` from a deterministic key (`SigningKey::from_bytes(&[1u8; 32])`). Re-pin both files in lockstep.

## Output artefacts

After a successful `aiken build` the validators are emitted to `onchain/plutus.json`:

| Title | Type | Used as |
|---|---|---|
| `governance_nft.governance_nft.mint` | minting policy | inlined by `bootstrap_governance` |
| `oracle.oracle.withdraw` | withdrawal validator | published as a reference script by `publish_scripts`, attached by `consume_oracle_data` |

## License

Licensed under the Apache License, Version 2.0. See `LICENSE`.
