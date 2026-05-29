# Testing — Shipping Oracle

Runbook for reproducing every test in the project, from unit tests up to a full local end-to-end on-chain flow. Designed to be followed top-to-bottom from a clean checkout.

## Matrix

| Level | What it tests | Command | Network / account | Time |
|---|---|---|---|---|
| 1 | Unit + integration (Rust) | `cargo test` | none (Shippo stubbed via wiremock) | ~30 s |
| 2 | Live API smoke | `cargo run` + `scripts/smoke.sh` | real Shippo | ~5 s |
| 3 | On-chain unit tests (Aiken) | `aiken check` | none | ~2 s |
| 4 | On-chain build (two-pass) | `aiken build` ×2 | none | ~5 s |
| 5 | End-to-end on local devnet | `trix devnet` + tx3 | local Dolos (no testnet needed) | ~1 min |
| 6 | Rust SDK consumer + escrow flow | `cd sdk/rust && cargo test --all-targets -- --nocapture` | none (backend + Shippo stubbed) | ~10 s |

---

## 1. Backend tests (no network)

```bash
cd backend
cargo test                             # everything
cargo test --test cbor_alignment       # CBOR alignment vs Aiken
cargo test --test signature_vectors    # deterministic Ed25519 vectors
cargo test --test integration          # HTTP end-to-end with wiremock
cargo test --test integration_report   # generates backend/reports/integration.{json,md}
```

No `.env` required — Shippo is stubbed.

The `integration_report` test exercises every status path (DELIVERED, IN_TRANSIT, PRE_TRANSIT, NOT_DELIVERED via FAILURE/RETURNED, UNKNOWN, plus the upstream-error path) and writes a JSON + markdown report to `backend/reports/`. CI uploads both as artifacts (see `.github/workflows/integration.yml`) — they're the milestone evidence E2.

---

## 2. Live API smoke

### 2.1 Configure `.env`

```bash
cd backend
cp .env.example .env
```

Fill in the five required variables:

| Variable | Where it comes from |
|---|---|
| `SHIPPO_API_KEY` | Shippo dashboard |
| `ORACLE_SK` | 32-byte hex (`openssl rand -hex 32`, or the signing key of an existing wallet) |
| `ORACLE_PKH` | blake2b-224 of the vkey (28 bytes hex) |
| `ORACLE_ADDRESS` | `addr_test1…` you control (or any placeholder for smoke testing) |
| `TRP_URL` | TRP endpoint, e.g. `http://localhost:8164` for the trix devnet |

### 2.2 Start the server

```bash
cd backend
cargo run
# → "oracle listening addr=0.0.0.0:3000"
```

### 2.3 Run the smoke script

In another terminal:

```bash
./scripts/smoke.sh
# Optional overrides:
#   BASE_URL=http://host:port ./scripts/smoke.sh
#   CARRIER=ups ./scripts/smoke.sh   (real tracking numbers, not the demo ones)
```

The script hits `/health` and `/v1/shipment` with three Shippo demo tracking numbers: `SHIPPO_PRE_TRANSIT`, `SHIPPO_TRANSIT`, `SHIPPO_DELIVERED`.

Requires `jq`.

---

## 3. On-chain unit tests

```bash
cd onchain
aiken check                   # all
aiken check -m oracle         # withdrawal validator only
aiken check -m governance_nft # mint policy only
```

Coverage:

- `oracle.ak`: `withdraw_valid_signature`, `withdraw_invalid_signature`, `withdraw_tampered_data`, `withdraw_missing_governance_nft`.
- `escrow.ak`: delivered release and timeout refund scenarios for the ADA escrow template.
- `governance_nft.ak`: `mint_valid`, `mint_missing_seed_input`, `mint_wrong_asset_name`, `mint_wrong_quantity`, `mint_extra_asset`.
- `cbor_alignment_tests.ak`: byte vectors pinned against `backend/tests/cbor_alignment.rs`.

If you change a vector on the Rust side, the Aiken side must be updated at the same time — the hex strings are hardcoded in both files.

---

## 4. On-chain build (two-pass)

There's a mandatory order: `oracle.ak` references `config.gov_policy_id`, and that hash depends on the compiled bytecode of `governance_nft.ak`. So you compile twice.

```bash
cd onchain

# Pass 1: compile against whatever placeholder is in aiken.toml
aiken build

# Capture the real policy id
GOV_POLICY_ID=$(jq -r '.validators[] | select(.title|contains("governance_nft.mint")) | .hash' plutus.json)
echo "$GOV_POLICY_ID"

# Replace gov_policy_id in aiken.toml [config.default]
# Keep encoding = "base16"
# Edit manually or:
#   sed -i.bak "s/^gov_policy_id = .*/gov_policy_id = { bytes = \"$GOV_POLICY_ID\", encoding = \"base16\" }/" aiken.toml

# Pass 2: recompile against the real policy id
aiken build
```

Reminders:

- In `aiken.toml`, byte values must be `{ bytes = "...", encoding = "base16" }`. Omitting `encoding` makes `aiken check` fail with `missing field encoding`.
- Every time you change `seed_utxo_*`, the `gov_policy_id` changes → repeat the two-pass.

---

## 5. End-to-end on the local devnet (Dolos)

> Exact `trix` flags vary between versions. If a command doesn't match, run `trix --help` or `trix <subcommand> --help`.

### 5.1 Start the devnet

```bash
cd tx3
trix devnet start
trix devnet info     # shows pre-funded wallets (alice/bob/charlie)
```

`devnet.toml` defines pre-funded UTxOs for `@alice`, `@bob`, `@charlie` (100k ADA each).

### 5.2 Provision the oracle wallet

Use one of the pre-funded devnet wallets, or create a fresh one:

```bash
cshell wallet create oracle
```

Export its signing-key hex into `backend/.env::ORACLE_SK` so the API signs with the same key the on-chain governance UTxO will trust. The wallet's verification key (hex, 32 bytes) goes into `tx3/.env.local::ORACLE_VK`.

### 5.3 Wire `seed_utxo_ref` and rebuild

```bash
trix utxos --wallet oracle    # pick one; copy tx_hash and index
# Edit onchain/aiken.toml:
#   seed_utxo_tx_hash = { bytes = "<tx_hash>", encoding = "base16" }
#   seed_utxo_index   = <index>
cd onchain && aiken build     # pass 1 → new gov_policy_id
# Update gov_policy_id in aiken.toml (see section 4)
aiken build                   # pass 2
```

### 5.4 Publish the scripts

```bash
cd tx3
trix invoke -p local           # choose: publish_scripts
```

Publishes `governance_nft`, `oracle`, and `escrow` as reference scripts. Note the refs (`txhash#ix`) it returns and set them in `tx3/.env.local::ORACLE_SCRIPT_REF` and `tx3/.env.local::ESCROW_SCRIPT_REF`.

### 5.5 Bootstrap governance (mint the NFT)

```bash
trix invoke -p local           # choose: bootstrap_governance
```

Consumes `seed_utxo_ref`, mints the NFT, and locks it in a UTxO with `GovernanceDatum { oracle_vk }`. Note the resulting `governance_utxo_ref` and set it in `tx3/.env.local::GOVERNANCE_UTXO_REF`.

### 5.6 Run the backend against the devnet

In `backend/.env`:

```
TRP_URL=http://localhost:8164    # devnet TRP endpoint
ORACLE_SK=<same sk as the oracle wallet>
```

```bash
cd backend && cargo run
./scripts/smoke.sh    # confirm the API responds
```

### 5.7 Consumer tx (consume_oracle_data)

Recommended: use the Rust SDK example to fetch the attestation, verify it, and write the tx3 args JSON in one step.

```bash
cd sdk/rust
ORDER_ID=ord_123 \
TX3_ARGS_OUT=/tmp/consume_args.json \
cargo run --example e2e_consume_oracle

cd ../../tx3
trix invoke -p local --args-json-path /tmp/consume_args.json    # choose: consume_oracle_data
```

Manual fallback if you want to inspect the raw HTTP response:

```bash
RESPONSE=$(curl -fsS "http://localhost:3000/v1/shipment?carrier=shippo&tracking_number=SHIPPO_DELIVERED")

cat > /tmp/consume_args.json <<EOF
{
  "p_carrier_hash":         "$(jq -r '.data.carrier_hash'         <<<"$RESPONSE")",
  "p_tracking_number_hash": "$(jq -r '.data.tracking_number_hash' <<<"$RESPONSE")",
  "p_status":               "$(jq -r '.data.status' <<<"$RESPONSE" | xxd -p | tr -d '\n')",
  "p_timestamp":            $(jq -r '.data.timestamp' <<<"$RESPONSE"),
  "p_signature":            "$(jq -r '.signature' <<<"$RESPONSE")"
}
EOF
```

### 5.8 Verify on-chain

```bash
trix utxos --wallet consumer    # the `attested` UTxO with OracleData inline should appear
trix tx <txhash>                # tx details
```

If the signature doesn't validate, the script fails and the tx is rejected by the node — that's the on-chain smoke test.

### 5.9 Escrow template transactions

The Milestone 3 ADA escrow tx3 templates are part of `tx3/main.tx3`:

- `lock_escrow_ada` locks buyer ADA at the escrow script for one order/shipment pair.
- `release_escrow` spends the escrow to `Merchant` when the oracle attests `DELIVERED`.
- `refund_escrow` spends the escrow back to `Buyer` after `refund_after`.

Before running the escrow flow, set `BUYER` and `MERCHANT` party addresses in `tx3/.env.local`. The `buyer_pkh` and `merchant_pkh` args must be the 28-byte payment key hashes for those same addresses.

If you have cardano-cli compatible payment vkey files, derive them with:

```bash
cardano-cli address key-hash --payment-verification-key-file buyer.vkey
cardano-cli address key-hash --payment-verification-key-file merchant.vkey
```

#### 5.9.1 Lock escrow for a delivered shipment

Use the SDK example to fetch and verify the attestation, link it to the order id, and write the `lock_escrow_ada` tx3 args JSON:

```bash
cd ../sdk/rust
BUYER_PKH=<28-byte buyer payment key hash hex> \
MERCHANT_PKH=<28-byte merchant payment key hash hex> \
ORDER_ID=ord_escrow_release_001 \
ESCROW_LOVELACE=10000000 \
LOCK_ESCROW_ARGS_OUT=/tmp/lock_escrow_release_args.json \
cargo run --example e2e_escrow_flow
```

Submit the lock transaction:

```bash
cd ../../tx3
trix invoke -p local --args-json-path /tmp/lock_escrow_release_args.json    # choose: lock_escrow_ada
```

Capture the UTxO locked at the escrow script from the command output or by querying the buyer/escrow address. Export it as `<lock_tx_hash>#<output_index>`:

```bash
export ESCROW_UTXO=<lock_tx_hash>#<output_index>
```

#### 5.9.2 Release escrow after DELIVERED

Rerun the SDK example with `ESCROW_UTXO`; it will write both release and refund args for that locked UTxO:

```bash
cd ../sdk/rust
BUYER_PKH=<28-byte buyer payment key hash hex> \
MERCHANT_PKH=<28-byte merchant payment key hash hex> \
ORDER_ID=ord_escrow_release_001 \
ESCROW_UTXO=$ESCROW_UTXO \
RELEASE_ESCROW_ARGS_OUT=/tmp/release_escrow_args.json \
REFUND_ESCROW_ARGS_OUT=/tmp/refund_escrow_args.json \
cargo run --example e2e_escrow_flow
```

Submit the release transaction:

```bash
cd ../../tx3
trix invoke -p local --args-json-path /tmp/release_escrow_args.json    # choose: release_escrow
```

Expected result: the escrow input is spent and the funds are paid to `MERCHANT`. If you change `SHIPMENT_TRACKING_NUMBER` to a non-delivered demo tracking number, the validator rejects the release because `status != DELIVERED`.

#### 5.9.3 Refund escrow after timeout

Use a separate escrow UTxO for the refund path. Set `REFUND_AFTER=0` so the timeout is immediately satisfied on the local devnet:

```bash
cd ../sdk/rust
BUYER_PKH=<28-byte buyer payment key hash hex> \
MERCHANT_PKH=<28-byte merchant payment key hash hex> \
ORDER_ID=ord_escrow_refund_001 \
ESCROW_LOVELACE=10000000 \
REFUND_AFTER=0 \
LOCK_ESCROW_ARGS_OUT=/tmp/lock_escrow_refund_args.json \
cargo run --example e2e_escrow_flow

cd ../../tx3
trix invoke -p local --args-json-path /tmp/lock_escrow_refund_args.json    # choose: lock_escrow_ada
export REFUND_ESCROW_UTXO=<refund_lock_tx_hash>#<output_index>
```

Generate and submit the refund args:

```bash
cd ../sdk/rust
BUYER_PKH=<28-byte buyer payment key hash hex> \
MERCHANT_PKH=<28-byte merchant payment key hash hex> \
ORDER_ID=ord_escrow_refund_001 \
REFUND_AFTER=0 \
ESCROW_UTXO=$REFUND_ESCROW_UTXO \
REFUND_ESCROW_ARGS_OUT=/tmp/refund_escrow_args.json \
cargo run --example e2e_escrow_flow

cd ../../tx3
trix invoke -p local --args-json-path /tmp/refund_escrow_args.json    # choose: refund_escrow
```

Expected result: the escrow input is spent and the funds are paid back to `BUYER`. If `refund_after` is in the future, the validator rejects the refund until the transaction validity range starts after that timeout.

The on-chain behavior is covered by `aiken check` tests in `onchain/validators/escrow.ak`. The tx3 protocol shape is checked with:

```bash
cd tx3
trix check
```

---

## 6. Rust SDK tests

```bash
cd sdk/rust
cargo test --all-targets -- --nocapture
```

The SDK suite spins up the existing backend server logic locally, stubs Shippo via `wiremock`, and verifies the consumer-side and escrow argument-generation flows end-to-end:

- `tests/client.rs` checks health, typed fetches, context-linked commitment preparation, and tx3 args for `consume_oracle_data`, `lock_escrow_ada`, `release_escrow`, and `refund_escrow`.
- `tests/verification.rs` checks expected-key pinning and tamper detection.
- `tests/report.rs` writes `sdk/rust/reports/sdk-integration.{json,md}` with tx3-ready args for every supported status path plus the upstream-error case.

CI uploads both report files as workflow artifacts from `.github/workflows/sdk.yml`.

---

## Gotchas

1. **Skipping the second `aiken build`** after changing `gov_policy_id` or `seed_utxo_*` in `aiken.toml` → on-chain compiles with the placeholder and fails at runtime.
2. **`oracle_vk` out of sync** between `backend/.env` (`ORACLE_SK`) and the `GovernanceDatum` minted in 5.5 → the signature verifies cryptographically but the validator rejects it because the vk in the datum isn't the one that signed. Re-mint governance, or reconfigure `ORACLE_SK`.
3. **`status` as string vs bytes**: the field is `Bytes` on-chain. The API returns a string (`"DELIVERED"`), but the consumer needs the hex of the UTF-8 bytes. Hence the `xxd -p` in 5.7.
   The Rust SDK example handles this conversion automatically.
4. **Out-of-sync signature vectors**: if you touch `signature_vectors.rs`, you must re-paste `test_oracle_vk` and `test_oracle_sig` into `onchain/validators/oracle.ak` (lines 50-54). Same for `cbor_alignment_tests.ak` when the `OracleData` hex changes.
