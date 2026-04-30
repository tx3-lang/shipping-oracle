# Milestone 2: Off-chain Data Fetcher and On-chain Submission

## Context

The shipping oracle currently uses a **push-based** architecture where the backend polls the blockchain for tracking UTxOs on a cron schedule, queries Shippo, and submits close_shipment transactions. Milestone 2 requires re-architecting to a **pull-based** model (inspired by Pyth) where:

- The oracle exposes an HTTP API that consumers call
- The oracle returns **data + Ed25519 signature**
- The consumer builds their own transaction, attaching the oracle's reference script via the **withdrawal trick**
- A static governance UTxO holds the oracle's verification key
- On-chain data must use **hashed identifiers** (no PII exposure, per milestone acceptance criteria B1)

### Milestone Acceptance Criteria (from Project Catalyst)
- A1: Off-chain fetcher retrieves shipment status from logistics API
- B1: Oracle contract stores hashed shipment data without exposing PII
- C1: Integrated flow results in accurate on-chain submissions
- D1: Documentation describing the workflow is publicly available
- Evidence: source code, video walkthrough, example on-chain txs, test reports

---

## Architecture Overview

Diagrams are authored in PlantUML under [`diagrams/`](../diagrams/). To regenerate PNGs after editing the `.puml` sources, run `./diagrams/build.sh` (requires Docker).

### C4 Container Diagram

![C4 Container](../diagrams/milestone-2-c4-container.png)

Source: [`diagrams/milestone-2-c4-container.puml`](../diagrams/milestone-2-c4-container.puml)

### Sequence Diagram (pull-based flow)

![Pull-based sequence](../diagrams/milestone-2-sequence.png)

Source: [`diagrams/milestone-2-sequence.puml`](../diagrams/milestone-2-sequence.puml)

---

## Implementation Plan

### Phase 1: On-chain Layer (Aiken)

#### 1.1 Rewrite `onchain/lib/types.ak`

Remove: `TrackingDatum`, `ShipmentDatum`, `TrackingRedeemer`

New types:
```aiken
type GovernanceDatum {
  oracle_vk: ByteArray,  // Ed25519 verification key (32 bytes, full key not hash)
}

type OracleData {
  carrier_hash: ByteArray,          // blake2b hash of carrier name
  tracking_number_hash: ByteArray,  // blake2b hash of tracking number
  status: ByteArray,                // "DELIVERED", "NOT_DELIVERED", "IN_TRANSIT", etc.
  timestamp: Int,                   // unix timestamp
}

type OracleRedeemer {
  data: OracleData,
  signature: ByteArray,  // Ed25519 signature over serialise_data(data)
}
```

**Note**: Using hashed identifiers (carrier_hash, tracking_number_hash) satisfies milestone criteria B1 - no PII on-chain.

#### 1.2 New `onchain/validators/governance_nft.ak` (NEW - minting policy)

One-shot minting policy to produce a single, unforgeable token that identifies the authentic governance UTxO:

```aiken
validator governance_nft {
  mint(_redeemer, policy_id, tx: Transaction) {
    // One-shot: requires consuming a specific hardcoded UTxO (set in aiken.toml config)
    // This guarantees the policy can only mint once - NFT uniqueness
    expect list.has(tx.inputs, config.seed_utxo_ref)
    // Enforce: exactly 1 token minted, specific asset_name
    let minted = assets.tokens(tx.mint, policy_id)
    expect [(asset_name, 1)] = dict.to_list(minted)
    asset_name == config.governance_asset_name
  }
  else(_) { fail }
}
```

#### 1.3 New `onchain/validators/oracle.ak` (replaces `tracking.ak`)

Withdrawal validator using the withdrawal trick:

```aiken
validator oracle {
  withdraw(redeemer: OracleRedeemer, _account, tx: Transaction) {
    // 1. Find governance UTxO in reference_inputs by looking for the governance NFT
    //    (hardcoded policy_id + asset_name from config). Only ONE UTxO can have this token.
    expect Some(gov_input) = list.find(
      tx.reference_inputs,
      fn(input) { assets.quantity_of(input.output.value, config.gov_policy_id, config.gov_asset_name) == 1 }
    )
    // 2. Extract GovernanceDatum with oracle_vk
    expect InlineDatum(gov_data) = gov_input.output.datum
    expect gov: GovernanceDatum = gov_data
    // 3. Serialize OracleData
    let message = builtin.serialise_data(redeemer.data)
    // 4. Verify Ed25519 signature
    builtin.verify_ed25519_signature(gov.oracle_vk, message, redeemer.signature)
  }

  else(_ctx) { fail @"unsupported purpose" }
}
```

**Why NFT-based identity (upgrade from hardcoded address):**
- One-shot minting policy → NFT is unforgeable by design
- Anyone can send UTxOs to the oracle's wallet address, but only ONE UTxO in the universe carries the governance NFT
- Governance rotation = move the NFT to a new UTxO (no validator recompile needed)

#### 1.4 Update `onchain/aiken.toml`

Replace config params:
- Remove: `tracking_price`, `payment_address`
- Add:
  - `seed_utxo_ref` (tx_hash#idx consumed by the one-shot mint)
  - `governance_asset_name` (hex-encoded asset name, e.g. "474f56" = "GOV")
  - `gov_policy_id` (policy hash of the governance_nft validator; set after compiling the mint policy)

Note: `gov_policy_id` creates a circular dependency (mint policy and oracle validator reference each other). Solve in two passes: compile governance_nft first → get its policy_id → write to config → compile oracle validator.

#### 1.5 Aiken tests

Write tests in the validator file or a test module:
- `test oracle_valid_signature()` - valid data+sig passes
- `test oracle_invalid_signature()` - tampered data fails
- `test oracle_missing_governance_nft()` - no ref input with NFT fails
- `test governance_nft_one_shot()` - mint requires seed UTxO, exactly one token

---

### Phase 2: TX3 Protocol Definitions

#### 2.1 Rewrite `tx3/main.tx3`

Three transactions (replaces the current three):

**`publish_scripts`** - One-time: publish both scripts as reference scripts
- Input: Oracle funds
- Output 1: Reference script output with `governance_nft` minting policy
- Output 2: Reference script output with `oracle` withdrawal validator
- Output 3: Change to Oracle

**`bootstrap_governance`** - One-time: mint NFT + create governance UTxO
- Input: Oracle funds (including the `seed_utxo_ref` that the one-shot policy requires)
- Mint: 1x governance NFT via `governance_nft` policy
- Output 1: Governance UTxO at Oracle address, contains the NFT + `GovernanceDatum { oracle_vk }`
- Output 2: Change to Oracle

**`consume_oracle_data`** - Example/demo consumer tx (for testing + documentation):
- Reference input: governance UTxO (carries the NFT)
- Reference script: oracle withdrawal validator
- Withdrawal: from oracle script reward address, 0 lovelace, redeemer = `OracleRedeemer { data, signature }`
- Input: Consumer funds
- Output: Consumer stores attested data in their own UTxO (demo purpose)

Parties: `Oracle`, `Consumer`
Env: `governance_nft_script_bytes`, `oracle_script_bytes`, `governance_nft_script_ref`, `oracle_script_ref`, `governance_utxo_ref`, `oracle_vk`, `gov_policy_id`, `gov_asset_name`

#### 2.2 Regenerate `backend/src/tx3.rs`

Run TX3 codegen after updating `main.tx3`.

---

### Phase 3: Backend Restructuring (Rust)

#### 3.1 Update `backend/Cargo.toml`

- **Add**: `axum`, `tower-http` (CORS), `blake2` (for hashing identifiers)
- **Remove**: `tokio-cron-scheduler`
- **Keep**: `tokio`, `reqwest`, `serde`, `serde_json`, `ed25519-dalek`, `pallas`, `tx3-sdk`, `chrono`, `hex`, `anyhow`

#### 3.2 Rewrite `backend/src/config.rs`

Remove: `cron_schedule`, `oracle_payment_address`, `blockfrost_url`, `validator_script_ref`
Add: `listen_address` (default `0.0.0.0:3000`)
Keep: `shippo_api_key`, `oracle_sk`, `oracle_pkh`, `oracle_address`, `trp_url`, `trp_api_key`

#### 3.3 New `backend/src/api.rs` - HTTP API

```
GET /v1/shipment?carrier={carrier}&tracking_number={tracking_number}
```

Response:
```json
{
  "data": {
    "carrier_hash": "abc...",
    "tracking_number_hash": "def...",
    "status": "DELIVERED",
    "timestamp": 1712000000
  },
  "plaintext": {
    "carrier": "usps",
    "tracking_number": "ABC123"
  },
  "signature": "hex...",
  "public_key": "hex...",
  "cbor_hex": "d8799f..."
}
```

- `plaintext` = convenience for consumer UX (not signed, not on-chain)
- `data` = hashed version, what gets signed and goes on-chain
- `cbor_hex` = exact CBOR bytes that were signed (consumer embeds directly in redeemer)
- `GET /health` - health check endpoint

#### 3.4 New `backend/src/oracle_service.rs` (replaces `fetcher.rs`)

Core logic:
1. Receive carrier + tracking_number from API request
2. Query Shippo for current status (using existing `ShipmentClient`)
3. Hash carrier and tracking_number with blake2b (for on-chain privacy)
4. Build `OracleData` as PlutusData Constr(0, [carrier_hash, tracking_number_hash, status, timestamp])
5. CBOR-serialize using `pallas::codec::minicbor` (must match Aiken's `builtin.serialise_data`)
6. Sign CBOR bytes with Ed25519
7. Return data + signature + cbor_hex

**Critical**: The CBOR encoding must exactly match what Aiken's `builtin.serialise_data` produces. This is the highest-risk technical challenge.

#### 3.5 Update `backend/src/shipment.rs`

Expand `get_status()` to return ALL statuses (not just final ones):
- DELIVERED -> "DELIVERED"
- FAILURE/RETURNED -> "NOT_DELIVERED"  
- TRANSIT -> "IN_TRANSIT"
- PRE_TRANSIT -> "PRE_TRANSIT"
- UNKNOWN -> "UNKNOWN"

#### 3.6 Update `backend/src/models.rs`

Remove: `TrackingUTxO`, `TrackingDatum`
Add: `OracleData`, `SignedOracleResponse`, `ShipmentQuery`
Keep: `TrackingResponse`, `TrackingStatus` (Shippo API models)

#### 3.7 Rewrite `backend/src/main.rs`

```rust
#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_env()?;
    let shipment_client = Arc::new(ShipmentClient::new(config.clone())?);
    let signing_key = load_signing_key(&config.oracle_sk)?;
    let oracle_service = Arc::new(OracleService::new(shipment_client, signing_key));
    let app = api::create_router(oracle_service);
    let listener = TcpListener::bind(&config.listen_address).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
```

#### 3.8 Update `backend/src/lib.rs`

Modules: `api`, `config`, `models`, `oracle_service`, `shipment`, `tx3`
Remove: `blockchain`, `fetcher`, `scheduler`, `submitter`

#### 3.9 Files to DELETE

- `backend/src/scheduler.rs` - no more polling
- `backend/src/submitter.rs` - oracle doesn't submit txs
- `backend/src/fetcher.rs` - replaced by oracle_service
- `backend/src/blockchain.rs` - replaced; signing logic moves to oracle_service
- `onchain/validators/tracking.ak` - replaced by oracle.ak

---

### Phase 4: Testing

#### 4.1 Aiken validator tests

Test with hardcoded (data, signature, vk) triples to verify on-chain signature verification works.

#### 4.2 CBOR serialization tests (Rust unit tests)

Serialize `OracleData` as PlutusData, verify hex output matches expected encoding. Cross-reference with Aiken's `serialise_data` for same values.

#### 4.3 Rewrite `backend/tests/integration.rs`

1. Start oracle HTTP server on random port
2. Send requests for Shippo test tracking numbers
3. Verify response contains valid data, signature, cbor_hex
4. Verify signature: deserialize cbor_hex, verify with ed25519_dalek
5. Generate test reports (milestone evidence E2)

#### 4.4 End-to-end local test with trix devnet (Dolos)

Fulfils the DoD "we can run locally the oracle (off-chain + on-chain)":

1. **Setup devnet**: `trix devnet start` (launches local Dolos node)
2. **Generate test keys**: Ed25519 keypair for oracle (stored in local `.env`)
3. **Compile on-chain**: `aiken build` (two-pass for mint policy → validator config)
4. **Publish scripts**: run `publish_scripts` tx3 against devnet
5. **Bootstrap governance**: run `bootstrap_governance` tx3 to mint NFT + create gov UTxO
6. **Start oracle backend**: `cargo run` → HTTP API listens locally
7. **Query API**: `curl localhost:3000/v1/shipment?carrier=usps&tracking_number=...`
8. **Consumer tx**: run `consume_oracle_data` tx3 with the signed data from the API
9. **Verify on-chain**: query devnet for the consumer tx, confirm it succeeded
10. **Generate test report**: integration tests produce JSON + markdown reports (milestone evidence E2)

This entire workflow runs **100% locally** - no Shippo account needed for smoke tests (use mocked `ShipmentClient` for devnet integration tests), no testnet faucet, no public network.

For milestone evidence C2 (example transactions in public blockchain explorer): additionally run the full flow once against Cardano preview testnet and capture tx hashes.

---

### Phase 5: Documentation & Milestone Evidence

#### 5.1 Update `README.md`

Document the pull-based architecture, API endpoints, how to run locally, how the consumer uses the oracle data.

#### 5.2 Update CI workflows

- `backend.yml` - adjust for new dependencies and removed modules
- `integration.yml` - adjust for HTTP API testing instead of scheduler-based testing

#### 5.3 Milestone evidence preparation

- A2: Public repo (already exists)
- B2: Video walkthrough (manual - outside scope of code)
- C2: Example transactions on preview testnet
- D2: Documentation in README
- E2: Test reports from integration tests

---

## Implementation Order

1. **CBOR alignment harness FIRST** - unit test that serializes `OracleData` in Rust (pallas) and compares against hex produced by Aiken `serialise_data`. Blocks everything else.
2. `onchain/lib/types.ak` - new data types
3. `onchain/validators/governance_nft.ak` - one-shot minting policy
4. `onchain/validators/oracle.ak` - withdrawal validator + tests
5. `onchain/aiken.toml` - config (two-pass: mint policy first, then validator)
6. `tx3/main.tx3` - publish_scripts, bootstrap_governance, consume_oracle_data
7. Regenerate `backend/src/tx3.rs`
8. `backend/Cargo.toml` - dependency changes (+ axum, blake2; - cron)
9. `backend/src/models.rs` - new data models
10. `backend/src/config.rs` - simplified config
11. `backend/src/shipment.rs` - expand status mapping
12. `backend/src/oracle_service.rs` - core signing logic (NEW)
13. `backend/src/api.rs` - HTTP API (NEW)
14. `backend/src/main.rs` - HTTP server entry point
15. `backend/src/lib.rs` - module declarations
16. Delete: `scheduler.rs`, `submitter.rs`, `fetcher.rs`, `blockchain.rs`, `tracking.ak`
17. `backend/tests/integration.rs` - rewrite tests
18. Devnet e2e script - bootstraps everything against trix devnet
19. `README.md` - documentation (off-chain + on-chain workflow, local run instructions)
20. CI workflows update

## Key Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| CBOR encoding mismatch off-chain vs on-chain | HIGH - signatures fail | Create shared test vectors, serialize same data in both Aiken and Rust, compare hex |
| Ed25519 compatibility (dalek vs Aiken builtin) | LOW - standard spec | Verify with test vectors early |
| TX3 withdrawal syntax | LOW (confirmed supported) | Verify exact syntax with TX3 docs |

## Files Summary

| File | Action |
|------|--------|
| `onchain/lib/types.ak` | REWRITE |
| `onchain/validators/tracking.ak` | DELETE |
| `onchain/validators/oracle.ak` | NEW |
| `onchain/validators/governance_nft.ak` | NEW (one-shot mint policy) |
| `onchain/aiken.toml` | UPDATE |
| `tx3/main.tx3` | REWRITE |
| `tx3/trix.toml` | UPDATE if needed |
| `backend/Cargo.toml` | UPDATE |
| `backend/src/main.rs` | REWRITE |
| `backend/src/lib.rs` | UPDATE |
| `backend/src/config.rs` | REWRITE |
| `backend/src/models.rs` | REWRITE |
| `backend/src/shipment.rs` | UPDATE (expand statuses) |
| `backend/src/oracle_service.rs` | NEW |
| `backend/src/api.rs` | NEW |
| `backend/src/tx3.rs` | REGENERATE |
| `backend/src/scheduler.rs` | DELETE |
| `backend/src/submitter.rs` | DELETE |
| `backend/src/fetcher.rs` | DELETE |
| `backend/src/blockchain.rs` | DELETE |
| `backend/tests/integration.rs` | REWRITE |
| `README.md` | UPDATE |
| `.github/workflows/*.yml` | UPDATE |
