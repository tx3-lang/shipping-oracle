# Shipping Oracle — Tx3

[tx3](https://github.com/tx3-lang/tx3) protocol describing the three Cardano transactions used by the **pull-based** Shipping Oracle: bootstrap the on-chain identity (publish scripts + mint the governance NFT) and consume signed attestations via the Pyth-style withdrawal trick.

## Overview

```
publish_scripts          (one-time, signed by Oracle)
   └─► publishes governance_nft + oracle as reference scripts

bootstrap_governance     (one-time, signed by Oracle, consumes the seed UTxO)
   └─► mints the governance NFT and locks it in a UTxO with
       inline datum GovernanceDatum { oracle_vk }

consume_oracle_data      (per-use, signed by Consumer)
   └─► reference_input  = governance UTxO   (carries the NFT)
       reference_script = oracle.withdraw   (from publish_scripts)
       withdrawal       = 0 ADA from the oracle script reward address
       redeemer         = OracleRedeemer { data, signature }   ← from the backend
       output           = consumer-owned UTxO with OracleData inline (demo)
```

The withdrawal trick lets the consumer attach the oracle validator without spending any UTxO governed by it — the validator runs because of the 0-lovelace withdrawal redeemer, and verifies the Ed25519 signature against the verification key in the governance UTxO.

## Files

- `main.tx3` — protocol definition: parties, env, datums (`GovernanceDatum`, `OracleData`, `OracleRedeemer`), and the three transactions.
- `trix.toml` — protocol metadata + codegen plugins.
- `devnet.toml` — pre-funded wallets used by the local Dolos devnet (`@alice`, `@bob`, `@charlie`).

## Parties

| Party | Used in | Role |
|---|---|---|
| `Oracle` | `publish_scripts`, `bootstrap_governance` | pays deposits, controls the seed UTxO and the governance NFT |
| `Consumer` | `consume_oracle_data` | pays fees, locks the attested data in their own UTxO |

## Env block (matches `env { ... }` in `main.tx3`)

Resolved by the active trix profile (`.env.preview`, `.env.dolos`, …).

| Variable | Source | Filled when |
|---|---|---|
| `ORACLE` (party address) | oracle wallet's bech32 address | once, before any tx |
| `CONSUMER` (party address) | consumer wallet's bech32 address | before `consume_oracle_data` |
| `GOVERNANCE_NFT_SCRIPT` | `plutus.json → governance_nft.mint.compiledCode` | after on-chain build |
| `ORACLE_SCRIPT` | `plutus.json → oracle.withdraw.compiledCode` (2nd pass) | after on-chain build |
| `ORACLE_SCRIPT_HASH` | reward address derived from `oracle.withdraw.hash` (header `0xf0` + hash, bech32 `stake_test`/`stake`) | after on-chain build |
| `ORACLE_SCRIPT_REF` | `<publish_tx_hash>#<idx>` of the oracle reference-script output | after `publish_scripts` |
| `ORACLE_VK` | oracle wallet vkey (32 bytes hex, no `5820` cbor prefix) | once |
| `GOV_POLICY_ID` | `plutus.json → governance_nft.mint.hash` | after on-chain build |
| `GOV_ASSET_NAME` | hex of the asset name (default `474f56` = `"GOV"`) | once |
| `SEED_UTXO_REF` | `<txhash>#<idx>` of an unspent UTxO controlled by `ORACLE` | once, before `bootstrap_governance` |
| `GOVERNANCE_UTXO_REF` | `<bootstrap_tx_hash>#<idx>` of the NFT-carrying output | after `bootstrap_governance` |

> The seed UTxO is hardcoded into the mint policy via `aiken.toml::seed_utxo_*` — the `SEED_UTXO_REF` env var must point at the **same** UTxO. Changing one without the other invalidates the whole flow.

## Running the transactions

End-to-end runbooks:

- **Local devnet (Dolos)** — see `TESTING.md` section 5. Note: `consume_oracle_data` is currently blocked on local Dolos by a `pallas-validate` phase-1 bug that rejects withdrawal redeemers (`UnneededRedeemer`); `publish_scripts` and `bootstrap_governance` work.
- **Preview testnet** — see `follow_steps.md` for the full path, including the patched local Dolos required for matching live cost models, the one-time stake-credential registration of the oracle script (deposit 2 ADA), and a working `consume_oracle_data` reference: tx `22b5939db715a6a59c544a9d45c38687fd57ca7d9a4429b85c64659999ea9bc0`.

## License

Licensed under the Apache License, Version 2.0. See `LICENSE`.
