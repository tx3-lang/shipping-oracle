# Run shipping-oracle locally

Clean step-by-step to compile, test, and exercise the full oracle (off-chain + on-chain) on your machine.

## Prerequisites

- **Rust** stable (≥ 1.70) — install via [rustup](https://rustup.rs).
- **Aiken** v1.1.21 — `aikup install v1.1.21`.
- **trix + dolos** — installed by [`tx3up`](https://github.com/tx3-lang/tx3up). Needed only for step 4 (on-chain demo).
- **Shippo API key** — sign up at [goshippo.com](https://goshippo.com). Needed only if you want live carrier data; the test suite uses stubs.
- `jq`, `curl`, `openssl`, `xxd` (preinstalled on macOS / most Linux).

```bash
git clone https://github.com/tx3-lang/shipping-oracle
cd shipping-oracle
```

---

## 1. Compile the on-chain validators

```bash
cd onchain
aiken check     # runs all unit tests + CBOR alignment vectors
aiken build     # emits plutus.json
```

`plutus.json` now contains the two validators used by the rest of the flow:

- `governance_nft.governance_nft.mint` — one-shot NFT minting policy
- `oracle.oracle.withdraw` — the withdrawal validator that verifies the oracle signature

> If you want a fresh deployment with your own NFT, repeat the build after pasting the new `governance_nft.mint.hash` into `aiken.toml::gov_policy_id`. The repo already ships with a working pair.

---

## 2. Run the backend test suite

```bash
cd ../backend
cargo test --all-targets
```

This exercises, end-to-end with no network:

- CBOR byte-level alignment between `pallas::minicbor` and Aiken `serialise_data`.
- Deterministic Ed25519 signature vectors shared with the on-chain tests.
- HTTP integration against every status path (Shippo stubbed via wiremock).
- An integration report harness that writes `backend/reports/integration.{json,md}` — open the markdown to inspect every case (carrier, status, signature, CBOR alignment, HTTP code).

---

## 3. Start the HTTP oracle

Create your `.env`:

```bash
cp .env.example .env
```

Fill in the five required variables:

| Variable | Value |
|---|---|
| `ORACLE_SK`      | `openssl rand -hex 32` (32-byte hex; this is the Ed25519 signing key) |
| `SHIPPO_API_KEY` | your Shippo token |
| `ORACLE_PKH`     | any 28-byte hex placeholder (only used by tx3 codegen wiring) |
| `ORACLE_ADDRESS` | any `addr_test1…` you control, or a placeholder for local-only smoke testing |
| `TRP_URL`        | `http://localhost:8164` (filled in step 4) or leave for now |

Run the server:

```bash
cargo run
# → oracle listening addr=0.0.0.0:3000
```

In another terminal, smoke-test the API:

```bash
./scripts/smoke.sh
# or directly
curl 'http://localhost:3000/v1/shipment?carrier=shippo&tracking_number=SHIPPO_DELIVERED' | jq
```

You should see a JSON payload with `data`, `plaintext`, `signature`, `public_key`, and `cbor_hex`. The `cbor_hex` are the exact bytes the oracle signed — consumers embed them verbatim in their on-chain redeemer.

---

## 4. End-to-end on-chain demo (trix devnet)

Spin up a local Cardano devnet via trix. From the repo root:

```bash
cd tx3
trix devnet start
trix devnet info       # shows pre-funded wallets
```

Pick a wallet from `trix devnet info` for the oracle party (or create one with `cshell wallet create`). Export its signing-key hex into `backend/.env::ORACLE_SK` so the API signs with the same key the on-chain governance UTxO will trust.

Create a `.env.local` next to `tx3/main.tx3` filling the env block from the freshly built `plutus.json`:

| Variable | Where it comes from |
|---|---|
| `ORACLE`                | bech32 address of the oracle wallet |
| `GOVERNANCE_NFT_SCRIPT` | `plutus.json → governance_nft.mint.compiledCode` |
| `ORACLE_SCRIPT`         | `plutus.json → oracle.withdraw.compiledCode` |
| `ORACLE_SCRIPT_HASH`    | reward address from `oracle.withdraw.hash` (header `0xf0` + hash, bech32 `stake_test`) |
| `ORACLE_VK`             | verification key of the oracle wallet (32 bytes hex) |
| `GOV_POLICY_ID`         | `plutus.json → governance_nft.mint.hash` |
| `GOV_ASSET_NAME`        | `474f56`  (ASCII `"GOV"`) |
| `SEED_UTXO_REF`         | `<txhash>#<idx>` of any unspent UTxO of the oracle wallet |

Publish the scripts and bootstrap the governance NFT:

```bash
trix invoke -p local           # choose: publish_scripts
# Note the output index of the oracle reference script and set:
#   ORACLE_SCRIPT_REF=<publish_tx>#<idx>

trix invoke -p local           # choose: bootstrap_governance
# Note the output that carries the GOV NFT and set:
#   GOVERNANCE_UTXO_REF=<bootstrap_tx>#<idx>
```

Build the consumer args from the live API response, then submit:

```bash
RESPONSE=$(curl -fsS 'http://localhost:3000/v1/shipment?carrier=shippo&tracking_number=SHIPPO_DELIVERED')

cat > /tmp/consume_args.json <<EOF
{
  "p_carrier_hash":         "$(jq -r '.data.carrier_hash'         <<<"$RESPONSE")",
  "p_tracking_number_hash": "$(jq -r '.data.tracking_number_hash' <<<"$RESPONSE")",
  "p_status":               "$(jq -r '.data.status' <<<"$RESPONSE" | xxd -p | tr -d '\n')",
  "p_timestamp":            $(jq -r '.data.timestamp' <<<"$RESPONSE"),
  "p_signature":            "$(jq -r '.signature' <<<"$RESPONSE")"
}
EOF

trix invoke -p local --args-json-path /tmp/consume_args.json   # choose: consume_oracle_data
```

The submitted transaction attaches the oracle validator via the withdrawal trick (0-lovelace withdrawal from the oracle script's reward address); the validator finds the governance UTxO via its NFT, reads the oracle verification key from its inline datum, and verifies the Ed25519 signature against the canonical CBOR of `OracleData`.

After the tx confirms, query the consumer wallet — you'll see a UTxO with the attested `OracleData` inline.

---

## What you've just done

- Built and tested both halves of the oracle locally.
- Run a real HTTP API that fetches shipment status, hashes the identifiers (no PII), and returns a signed attestation.
- Bootstrapped the on-chain identity (publish scripts + mint the governance NFT).
- Submitted a Pyth-style consumer transaction that verifies the oracle's signature on-chain.

For the complete test runbook (CBOR alignment, signature vectors, on-chain unit tests, CI-generated reports), see `TESTING.md`. For architecture and API reference, see `README.md`.
