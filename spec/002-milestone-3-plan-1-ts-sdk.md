# TypeScript SDK Implementation Plan (Milestone 3 — Plan 1 of 2)

> **For the coding agent:** This plan describes **goals, acceptance criteria, and constraints** — not line-by-line code. Implement each task idiomatically; the acceptance criteria (especially the pinned CBOR vectors) are the contract you must satisfy. Use TDD: write the acceptance test, see it fail, implement, see it pass.
>
> **Commits:** the USER runs `git commit` (project convention). At each commit point, hand over the files to stage and a message — do not run `git commit`. `.wolf/*` is gitignored — never stage it.

**Goal:** Build a TypeScript SDK in `sdk/typescript/` that fetches a Shipping Oracle attestation over HTTP and verifies it (Ed25519 + canonical CBOR + blake2b-256) **byte-for-byte identically to the Rust SDK and the Aiken validator**, so the e-commerce keeper (Plan 2) can trust the oracle's `DELIVERED`/`IN_TRANSIT` events.

**Architecture:** A small, dependency-light ESM package. The CBOR encoder must reproduce the exact `Constr(121, indefinite-array)` shape that pallas `minicbor` and Aiken `builtin.serialise_data` produce — proven against three pinned vectors. Verification uses `@noble/curves` (Ed25519) and `@noble/hashes` (blake2b). The HTTP client mirrors the Rust SDK's `OracleClient` surface.

**Tech Stack:** TypeScript (ESM, NodeNext), Node 18+ global `fetch`, `@noble/curves`, `@noble/hashes`, Vitest. **No CBOR library** — hand-roll the encoder for byte-exact control.

---

## Source of truth (read these before implementing)

The TS SDK is a port; match these exactly:

- `sdk/rust/src/verify.rs` — the verification algorithm (the contract for `verify`).
- `sdk/rust/src/client.rs` / `src/models.rs` — the API surface and wire types.
- `sdk/rust/README.md` — behavior to mirror in the TS README.
- `backend/tests/cbor_alignment.rs` + `onchain/lib/cbor_alignment_tests.ak` — the pinned CBOR vectors below.

### The verification algorithm (must match `verify.rs` step-for-step)

1. Decode `public_key` (must be 32 bytes); if a pinned key is configured, it must equal `public_key`.
2. Ed25519-verify `signature` (64 bytes) over the **raw `cbor_hex` bytes** (never re-derive/normalize `cbor_hex`).
3. Re-encode `data` to CBOR and assert it equals `cbor_hex` byte-for-byte.
4. `blake2b-256(plaintext.carrier)` == `data.carrier_hash` and `blake2b-256(plaintext.tracking_number)` == `data.tracking_number_hash`.

Each failure must throw a distinct, identifiable error (see Task 2 for the codes).

### Pinned CBOR vectors (the hard contract for the encoder)

`encodeOracleDataCbor(data)` MUST produce exactly these hex strings:

| status | carrier_hash | tracking_hash | timestamp | expected cbor hex |
|---|---|---|---|---|
| `DELIVERED` | `11`×32 | `22`×32 | 1712000000 | `d8799f5820`+`11`×32+`5820`+`22`×32+`4944454c4956455245441a660b0c00ff` |
| `UNKNOWN` | `33`×32 | `44`×32 | 1712345678 | `d8799f5820`+`33`×32+`5820`+`44`×32+`47554e4b4e4f574e1a6610524eff` |
| (empty)/empty hashes | `` | `` | 0 | `d8799f40404000ff` |

CBOR shape these imply (the encoder's spec): tag 121 = `0xd8 0x79`; indefinite array = `0x9f … 0xff`; byte string = major type 2 (`0x40|len` for len<24, `0x58 len` for 24–255, …); unsigned int = major type 0 in **canonical/shortest** form (`n`<24 inline, then `0x18`,`0x19`,`0x1a`,`0x1b`); negative int = major type 1 over `-1-n`. `status` bytes are its UTF-8 encoding; hashes are decoded from their hex.

---

## File Structure

| File | Responsibility |
|---|---|
| `sdk/typescript/package.json`, `tsconfig.json` | Package manifest + TS config (emit `dist/`) |
| `sdk/typescript/src/types.ts` | Wire types (`OracleAttestation`, `OracleData`, `OracleStatus`, `PreparedCommitment`, `HealthResponse`) |
| `sdk/typescript/src/error.ts` | `OracleSdkError` + error codes |
| `sdk/typescript/src/cbor.ts` | `encodeOracleDataCbor` + CBOR primitives |
| `sdk/typescript/src/verify.ts` | `verifyAttestation()` |
| `sdk/typescript/src/client.ts` | `OracleClient` |
| `sdk/typescript/src/index.ts` | Public exports |
| `sdk/typescript/test/*` | `cbor.test.ts`, `verify.test.ts`, `client.test.ts`, shared `helpers.ts` |
| `sdk/typescript/README.md`, `examples/order-commitment.ts` | Docs + A1 example |

Keep wire types **snake_case** (matching the HTTP JSON) so `cbor_hex`/`public_key`/`signature` are never reshaped before verification.

---

## Task 1: Scaffold the package

**Goal:** A buildable, testable ESM TypeScript package skeleton.

**Acceptance criteria:**
- `pnpm install` succeeds in `sdk/typescript/`.
- `pnpm build` (tsc → `dist/`) and `pnpm test` (Vitest) scripts exist and run (even with zero tests yet).
- Package is ESM (`"type": "module"`), `engines.node >= 18`, emits declarations.

**Constraints:** runtime deps limited to `@noble/curves` + `@noble/hashes`; dev deps `typescript` + `vitest`. Module/resolution = NodeNext, `strict: true`.

**Commit (hand to user):** stage `package.json tsconfig.json pnpm-lock.yaml`; message `chore(sdk-ts): scaffold TypeScript SDK package`.

---

## Task 2: Wire types and error type

**Goal:** Shared types and a single typed error.

**Acceptance criteria:**
- `OracleData` (`carrier_hash`, `tracking_number_hash`, `status`, `timestamp`), `ShipmentPlaintext`, `OracleAttestation` (`data`, `plaintext`, `signature`, `public_key`, `cbor_hex`), `HealthResponse`, `PreparedCommitment<T>` declared, matching `sdk/rust/src/models.rs`.
- `OracleStatus` is the union `'DELIVERED' | 'NOT_DELIVERED' | 'IN_TRANSIT' | 'PRE_TRANSIT' | 'UNKNOWN'`.
- `OracleSdkError extends Error` carries a `code` from a `OracleSdkErrorCode` union covering at least: `API`, `INVALID_LENGTH`, `RESPONSE_MISMATCH`, `UNEXPECTED_PUBLIC_KEY`, `INVALID_SIGNATURE`, `CBOR_MISMATCH`, `CARRIER_HASH_MISMATCH`, `TRACKING_NUMBER_HASH_MISMATCH` (mirrors `sdk/rust/src/error.rs`).
- `pnpx tsc --noEmit` is clean (`pnpm exec tsc --noEmit` on pnpm ≥7 where `pnpx` is unavailable).

**Commit:** stage `src/types.ts src/error.ts`; message `feat(sdk-ts): add wire types and OracleSdkError`.

---

## Task 3: CBOR encoder (parity harness — do this FIRST, it blocks everything)

**Goal:** `encodeOracleDataCbor(data): Uint8Array` that is byte-identical to pallas/Aiken.

**Acceptance criteria (the blocking test):**
- `test/cbor.test.ts` asserts all **three pinned vectors** above match exactly (encode → hex). This test is written first and must fail before the encoder exists, then pass.
- The encoder is hand-rolled (no CBOR dependency) and emits canonical/shortest integer forms.

**Constraints & gotchas:**
- Use `@noble/hashes/utils` `hexToBytes`/`bytesToHex` for hex (avoids hand-rolled hex bugs).
- `status` → UTF-8 bytes via `TextEncoder`; hashes → `hexToBytes(...)`.
- Integer encoding must be canonical (e.g. `1712000000` → `1a660b0c00`, `0` → `00`); verify against the vectors, don't trust a generic guess. Watch 32-bit sign issues — use unsigned shifts / `BigInt` for the 64-bit path.
- Empty byte strings encode as `0x40`; the zero-edge vector is the canary.

**Commit:** stage `src/cbor.ts test/cbor.test.ts`; message `feat(sdk-ts): canonical CBOR encoder with Rust/Aiken parity vectors`.

---

## Task 4: Attestation verification

**Goal:** `verifyAttestation(attestation, { expectedPublicKeyHex? })` implementing the 4-step algorithm above; throws `OracleSdkError` with the right `code` on each failure.

**Acceptance criteria:**
- A shared `test/helpers.ts` builds a fully valid, **self-signed** attestation with a deterministic key (sign the encoded CBOR with `@noble/curves` ed25519; compute the blake2b hashes for the plaintext) — so tests need no live backend.
- `test/verify.test.ts` covers: valid passes; correct pinned key passes; wrong pinned key → `UNEXPECTED_PUBLIC_KEY`; flipped signature → `INVALID_SIGNATURE`; mutated `data` (so re-encoded CBOR ≠ `cbor_hex`) → `CBOR_MISMATCH`; mutated `plaintext.carrier` → `CARRIER_HASH_MISMATCH`; thrown errors are `instanceof OracleSdkError`.

**Constraints & gotchas:**
- Ed25519: `@noble/curves/ed25519` → `ed25519.verify(sig, msg, pub)` (arg order!), `ed25519.sign`, `ed25519.getPublicKey`. Wrap `verify` in try/catch (malformed input must become `INVALID_SIGNATURE`, not a raw throw).
- blake2b-256: `@noble/hashes/blake2b` `blake2b(bytes, { dkLen: 32 })`. This matches Rust's `Blake2b::<U32>`.
- Compare hashes as lowercase hex (noble emits lowercase).
- Verify over the raw `cbor_hex` bytes; do not re-encode for the signature step (re-encoding is only for the separate CBOR-match check).

**Commit:** stage `src/verify.ts test/verify.test.ts test/helpers.ts`; message `feat(sdk-ts): verifyAttestation (ed25519 + cbor + blake2b)`.

---

## Task 5: OracleClient

**Goal:** `OracleClient` mirroring the Rust client: `health()`, `fetchAttestation(carrier, trackingNumber)`, `prepareCommitment(context, carrier, trackingNumber)`.

**Acceptance criteria:**
- Constructor: `new OracleClient(baseUrl, { expectedPublicKeyHex?, fetchFn? })`; trailing slashes trimmed from `baseUrl`; `fetchFn` defaults to global `fetch` and is **injectable** for tests.
- `fetchAttestation` → `GET {base}/v1/shipment?carrier=&tracking_number=`; non-2xx → `OracleSdkError('API', ...)` including status + body.
- `prepareCommitment` fetches, asserts the response `plaintext` matches the requested `carrier`/`tracking_number` (`RESPONSE_MISMATCH` otherwise), runs `verifyAttestation` (with the pinned key if configured), returns `{ context, attestation }`.
- `test/client.test.ts` (using an injected `fetchFn` returning a `Response`, reusing `helpers.ts`): fetch returns parsed attestation; prepareCommitment verifies + attaches context; wrong-shipment response → `RESPONSE_MISMATCH`; HTTP 502 → `OracleSdkError`.

**Commit:** stage `src/client.ts test/client.test.ts`; message `feat(sdk-ts): OracleClient (fetch + prepareCommitment)`.

---

## Task 6: Public exports + full build/test gate

**Goal:** A clean package entrypoint and a green full build.

**Acceptance criteria:**
- `src/index.ts` re-exports `OracleClient` (+ its options type), `verifyAttestation` (+ options), `encodeOracleDataCbor`, `OracleSdkError` (+ code type), and all wire types.
- `pnpm test` (or `pnpx vitest run`) → all tests green (cbor + verify + client).
- `pnpm build` → `dist/index.js` + `dist/index.d.ts`, no TS errors.

**Commit:** stage `src/index.ts`; message `feat(sdk-ts): public package entrypoint`.

---

## Task 7: README + runnable example

**Goal:** Docs and an A1 example linking an `order_id` to a verified tracking commitment.

**Acceptance criteria:**
- `examples/order-commitment.ts` constructs an `OracleClient` (base URL + optional pinned key from env), calls `prepareCommitment` with an order context, prints the linked order id + status. Mirrors the Rust `order_commitment` example.
- `README.md` documents: what it covers, install as a **git/path dependency** (publishing to the npm registry via `pnpm publish` is a later step), quick start, the API surface, the "never re-serialize `cbor_hex`" caution, and that the CBOR parity tests lock byte-compatibility with `backend/tests/cbor_alignment.rs` + the Aiken vectors. Mirror the Rust README's structure where sensible.

**Commit:** stage `examples/order-commitment.ts README.md`; message `docs(sdk-ts): README + order-commitment example`.

---

## Task 8 (optional): CI workflow + top-level README link

**Goal:** CI runs the SDK tests/build; the repo README points to the new SDK.

**Acceptance criteria:**
- `.github/workflows/sdk-ts.yml` runs `pnpm install`, `pnpm test`, `pnpm build` on `sdk/typescript/**` changes (Node 20, with `pnpm/action-setup`).
- Repo-root `README.md` SDKs section links `sdk/typescript/README.md`.

**Commit:** stage `.github/workflows/sdk-ts.yml README.md`; message `ci(sdk-ts): add TypeScript SDK workflow + README link`.

---

## Notes for the implementer

- **Deferred on purpose:** the tx3 args helper (`consume_oracle_data` params) from the Rust SDK is **out of scope** here — the escrow path uses the e-commerce escrow txs, not `consume_oracle_data` (per `spec/002-milestone-3.md`). Don't port it unless trivially free.
- **The whole point of this SDK is trust:** if `verify` ever passes a bad attestation or the CBOR drifts by a byte, the keeper (Plan 2) silently settles on forged data. The pinned-vector test (Task 3) and the verify tamper tests (Task 4) are the safety net — never weaken them.

---

## Self-Review (against `spec/002-milestone-3.md` Phase A.1)

- Public surface (`OracleClient`, `fetchAttestation`, `prepareCommitment`, `verify`) → Tasks 4–6. ✓
- `verify` reproduces Ed25519 + CBOR (`Constr 121` indefinite) + blake2b → Tasks 3–4, pinned to `verify.rs`. ✓
- Three pinned CBOR vectors as TS tests, FIRST → Task 3 (blocking). ✓
- Order-link example (A1) → Task 7. ✓
- tx-args helper deferred per spec → explicit note. ✓
- No code-transcription blocks (per user's plan-style preference); hard contracts kept as acceptance criteria/constraints. ✓

---

## Hand-off to Plan 2

Plan 2 (e-commerce keeper + e2e) is authored **after** this SDK lands, because its tasks depend on the final SDK API above and on e-commerce internals (`src/lib/cardano/escrow.ts` submit functions, `package.json`, the e2e harness) to be re-read at that point. Plan 2 scope: wire this SDK as a git/path dependency, build the poll-once `settle-escrows` keeper (`IN_TRANSIT`→`mark_shipped`, `DELIVERED`→`release`; refund stays buyer-initiated), and add the keeper-driven e2e tests on local dolos.
