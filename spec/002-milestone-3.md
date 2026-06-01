# Milestone 3: SDKs, Escrow Templates, Integration Examples & Documentation

## Context

Milestone 2 delivered the pull-based oracle: an HTTP API that returns shipment
`OracleData` + Ed25519 signature, a Rust SDK to fetch/verify it, and on-chain
validators (`oracle.ak` withdrawal validator + `governance_nft.ak`). Milestone 3
is about **making that oracle usable in real applications**: SDKs, escrow
templates that settle on the *delivered* event or a timeout, runnable end-to-end
integration examples, and documentation.

The e-commerce project at `../txpipe/tx3/e-commerce` already has a **deployed,
tested escrow** (Aiken `spend` validator + tx3 + off-chain TS + Supabase + e2e
tests). Its state machine is `Pending → Shipped → Released` (plus `Refund`), and
today every transition is **manual and time-based** — the merchant signs
`MarkShipped`, then signs `Release` after a grace period; the buyer signs
`Refund` after a ship deadline. **Nothing is connected to the shipping oracle.**
That bridge is the core of this milestone.

### Milestone Acceptance Criteria (from Project Catalyst)

- **A1**: SDKs let developers link orders/app events to on-chain shipment tracking commitments.
- **B1**: Escrow templates automatically trigger fund release or refund based on the *delivered* event or a timeout.
- **C1**: Integration examples are executable and demonstrate a full end-to-end flow using the oracle.
- **D1**: Documentation for SDKs, escrow templates, and integration examples is publicly available.
- **Evidence**: A2 public repo link · B2 test results (delivery + timeout scenarios) · C2 video walkthrough of the end-to-end flow · D2 documentation link.

---

## Design Decisions (from brainstorming)

These decisions were taken collaboratively and define the whole approach. They
are recorded here and in `.wolf/cerebrum.md` (Decision Log).

1. **Settlement model = off-chain keeper over the existing escrow (Option A).**
   The shipping oracle's `DELIVERED` event is enforced **off-chain** by a keeper
   that submits the existing escrow transactions. The escrow validator is **not
   modified** and **not redeployed**.
   - *Rationale:* (a) handles the real-world case where an order has **no
     shipment/tracking** (local pickup, in-person handoff) — a strictly
     on-chain oracle requirement would lock those funds forever; the keeper
     simply falls back to the manual flow. (b) Reuses everything already
     deployed and tested. (c) The escrow is a `spend` validator, so it is **not**
     affected by the `pallas-validate` withdrawal-redeemer bug that blocks the
     oracle's withdrawal trick on local dolos — the full e2e runs on local
     devnet. (d) Still satisfies B1: the release is *triggered by* the
     `DELIVERED` event; it is just enforced in the keeper rather than the
     validator.
   - *Trade-off (documented honestly in D):* the contract does not
     cryptographically verify the oracle signature; trust is in the keeper /
     merchant backend. The on-chain oracle-enforced escrow (verifying the
     attestation via the withdrawal trick) is a clean follow-up, not blocked by
     anything technical, but out of scope for this milestone.

2. **Both `IN_TRANSIT` and `DELIVERED` drive the escrow; each maps 1:1 to a
   deployed transition.** The escrow's two transitions map naturally onto the two
   oracle signals — no on-chain change, and the names line up:
   - **`IN_TRANSIT`** (oracle confirms the merchant dispatched) → `mark_shipped`
     (`Pending → Shipped`). This is the **merchant-protection** event: it
     **blocks the buyer refund** (refund requires `Pending`) and starts the grace
     window. *Why this and not `DELIVERED`:* if we only left `Pending` on
     delivery, a package still in transit past `ship_deadline` would let the
     buyer refund an order the merchant actually shipped — unfair, and
     `mark_shipped` on-chain even requires `shipped_at < ship_deadline`, so a late
     `DELIVERED` could fail the transition.
   - **`DELIVERED`** → the keeper submits `release` (the *settlement-on-delivery*
     event), additionally gated on-chain by `now >= grace_period_end`.
   - `PRE_TRANSIT` → traceability only (`order_events`), no escrow transition.

3. **`grace_period` = post-shipment dispute/transit window; semantics align with
   the deployed contract.** Because `mark_shipped` is driven by `IN_TRANSIT`,
   `grace_period_end = shipped_at + grace` keeps its original meaning. Release
   fires at `max(delivered_time, grace_period_end)`. The on-chain transition is
   correctly named `shipped`; the app/DB/UI additionally surface a `delivered`
   marker once the oracle reports `DELIVERED` (informational, drives the keeper's
   release decision).
   - *Dispute edge (out of scope):* shipped but never delivered (lost package) →
     escrow sits in `Shipped`, buyer can't refund (not `Pending`), keeper won't
     auto-release (no `DELIVERED`). The deployed contract still lets the merchant
     release after `grace_period_end` (timer-only on-chain check); richer dispute
     handling is out of scope.

4. **A TypeScript SDK is added** (ported from the Rust SDK) so the TS/Supabase
   e-commerce keeper consumes a first-class SDK rather than calling the HTTP API
   raw. Both SDKs are part of the public deliverable.

5. **Refund stays buyer-initiated; the keeper automates release only.** On-chain
   `Refund` requires the buyer's signature, so the keeper does **not** auto-submit
   refunds (no demo buyer key in the flow). The timeout is still enforced
   on-chain (the validator only allows `Refund` after `ship_deadline`); the buyer
   triggers it from their wallet via the existing `escrow-refund.ts`. Making
   refund fully automatic would require a *permissionless refund* validator
   change (anyone can trigger, funds still return to the buyer) — an on-chain
   change, out of scope here.

6. **SDK consumption = git/path dependency** from the e-commerce repo for now;
   publishing to npm is noted as the "publishable" follow-up step.

---

## Architecture Overview

```
                shipping-oracle repo (public)          e-commerce repo (public)
   ┌─────────────────────────────────────────┐   ┌──────────────────────────────────┐
   │  Backend HTTP API  (Milestone 2)         │   │  Keeper / settlement (NEW, TS)    │
   │    GET /v1/shipment → OracleData + sig    │◄──┤    poll-once (cron-style)         │
   │                                           │   │    per pending escrow:            │
   │  sdk/rust/        (exists, M2)            │   │      ├─ has tracking? → SDK fetch │
   │  sdk/typescript/  (NEW: fetch + verify)   │───┼─────►├─ IN_TRANSIT → mark_shipped  │
   └─────────────────────────────────────────┘   │      ├─ DELIVERED  → release        │
                                                  │      ├─ timeout    → flag for buyer │
                                                  │      └─ no tracking → manual flow  │
                                                  │   (refund = buyer-initiated)       │
                                                  │                                    │
                                                  │  aiken/ escrow (template, deployed)│
                                                  │    Pending → Shipped → Released    │
                                                  │                      ↘ Refund      │
                                                  │  scripts/ escrow-{release,refund,  │
                                                  │           mark-shipped}.ts (reuse) │
                                                  └──────────────────────────────────┘
```

A C4 container diagram + a sequence diagram (oracle-driven settlement) will be
authored in PlantUML under `diagrams/` (`milestone-3-*.puml`) and committed as
PNGs, consistent with the Milestone 2 convention.

---

## Implementation Plan

### Phase A — SDKs

#### A.1 New `sdk/typescript/` (port of the Rust SDK)

A small, dependency-light package mirroring `sdk/rust/`'s public surface, scoped
to what the keeper needs: **fetch + verify + read status**. The tx-args helper
is optional (the escrow path uses the e-commerce escrow txs, not
`consume_oracle_data`), so it is deferred unless trivial.

Public surface:

```ts
class OracleClient {
  constructor(baseUrl: string, opts?: { expectedPublicKeyHex?: string });
  // GET /v1/shipment?carrier=&tracking_number=
  fetchAttestation(carrier: string, trackingNumber: string): Promise<OracleAttestation>;
  // fetch + verify + return a typed, app-linked commitment
  prepareCommitment<T>(context: T, carrier: string, trackingNumber: string): Promise<PreparedCommitment<T>>;
}

interface OracleAttestation {
  data: OracleData;            // carrier_hash, tracking_number_hash, status, timestamp
  plaintext: ShipmentPlaintext;
  signature: string;           // hex
  publicKey: string;           // hex
  cborHex: string;
  verify(): void;              // throws on failure
}

type OracleStatus = 'DELIVERED' | 'NOT_DELIVERED' | 'IN_TRANSIT' | 'PRE_TRANSIT' | 'UNKNOWN';
```

`verify()` must reproduce the Rust SDK's checks **exactly**, or signatures will
silently disagree:
1. Ed25519 verification of `signature` over the raw `cbor_hex` bytes
   (`@noble/ed25519` or `@noble/curves`).
2. Reconstruct `OracleData` as PlutusData `Constr(0, [...])` with an
   **indefinite-length** field array (tag 121 = `0xd879`, `9f…ff`) and confirm
   the CBOR equals `cbor_hex` byte-for-byte. The e-commerce already encodes
   Plutus data this way in `src/lib/cardano/escrow.ts` (`CborTag(..., 121)`) —
   reuse the same CBOR library/approach.
3. `blake2b-256` of `plaintext.carrier` / `plaintext.tracking_number` must equal
   the hashes in `data` (`@noble/hashes/blake2b`, 32-byte output).
   - *Critical:* this must match the byte vectors pinned in
     `onchain/lib/cbor_alignment_tests.ak` and `backend/tests/cbor_alignment.rs`.
     Port at least the three pinned vectors (delivered / unknown / zero-edge) as
     TS unit tests so the TS `verify()` is proven equivalent to Rust + Aiken.

#### A.2 Rust SDK (`sdk/rust/`, exists)

No behavioral changes required. Add an example that demonstrates A1 explicitly —
linking an order id to a tracking commitment — and ensure the README documents
the same flow the TS SDK exposes (parity). Reuse the existing
`prepare_commitment` / `PreparedCommitment` API.

### Phase B — Escrow Template (reuse + document)

No on-chain changes. The e-commerce escrow **is** the template deliverable:

- `aiken/validators/escrow.ak`, `aiken/lib/escrow_types.ak` — `EscrowDatum
  { buyer, merchant, order_id, paid_at, ship_deadline, grace_period_end }`,
  redeemers `MarkShipped { shipped_at }` / `Release` / `Refund`.
- `tx3/main.tx3` — `lock_escrow_ada`, `mark_shipped`, `release_escrow`,
  `refund_escrow`.
- `scripts/escrow-{mark-shipped,release,refund}.ts` — the settlement mechanism
  the keeper reuses.

Deliverable here is **documentation that frames it as a reusable template** (see
Phase D): parties, datum/redeemer, the three transitions, the timeout
parameters (`ship_deadline`, `grace_period`), and how the oracle drives it.

### Phase C — Integration Example: the oracle-driven keeper (NEW)

A poll-once settlement runner in the e-commerce repo, in the style of the
existing CLI scripts (`scripts/`), runnable by cron. New file e.g.
`scripts/settle-escrows.ts` plus a small service module in
`src/lib/cardano/`.

Per pending escrow (read from the `escrows` + `orders` tables):

1. **No carrier/tracking** → skip oracle; leave to the manual merchant flow
   (fallback path; logged).
2. **Has carrier + tracking** → `OracleClient.fetchAttestation(carrier, tracking)`,
   `attestation.verify()`, then act on the status (decision 2):
   - `IN_TRANSIT` (dispatch confirmed): if escrow is `pending` → submit
     `mark_shipped` with `shipped_at = attestation.data.timestamp` (sets
     `grace_period_end = shipped_at + grace`; `Pending → Shipped`, which **blocks
     the buyer refund**). Record progress in `order_events`. *Note:* this must
     land before `ship_deadline` (on-chain `mark_shipped` requires `shipped_at <
     ship_deadline`); the keeper's poll cadence must be tighter than the ship
     window.
   - `DELIVERED`: record the `delivered` marker (app/DB + `order_events`); if
     escrow is `shipped` and `now >= grace_period_end` → submit `release` (reuse
     `submitReleaseEscrow`). (If the order was never seen `IN_TRANSIT`, first
     `mark_shipped`, then release once grace elapses.)
   - `PRE_TRANSIT`: record progress in `order_events` only; no escrow transition.
   - The **refund path is not automated by the keeper** (decision 5): refund is
     buyer-initiated via the existing `escrow-refund.ts`, gated on-chain by
     `ship_deadline` and only while still `pending` (i.e. the oracle never
     confirmed dispatch). The keeper may *flag* refund-eligible escrows for the
     buyer/UI, but does not submit.

The keeper is **idempotent** and **optimistic** (chain submit before DB write,
matching the existing scripts' Decision-Log A9 pattern): re-running it never
double-acts because it gates on the current escrow `status` + time windows.

DB: the escrow `status` stays `pending → shipped → released` (matching the
contract, driven by `IN_TRANSIT`/`release`). The `delivered` event from the
oracle is recorded as an app-layer marker / `order_events` row (decision 3) and
is what gates the keeper's release submission. Add a small Supabase
column/migration only if a delivered timestamp needs persisting.

### Phase D — Documentation

- **`docs/` integration guide** (in shipping-oracle, the public deliverable
  repo): end-to-end story — run the oracle, lock an escrow, keeper settles on
  `DELIVERED`, or refunds on timeout. Includes the **trust model** section
  (off-chain enforcement, decision 1 trade-off), the `IN_TRANSIT`→shipped /
  `DELIVERED`→release mapping (decision 2), and the **buyer-initiated refund**
  rationale + limitation (decision 5).
- **SDK docs**: `sdk/rust/README.md` (update) + `sdk/typescript/README.md`
  (new) with the link-order-to-tracking example (A1).
- **Escrow template doc**: datum/redeemer/transitions/timeouts reference
  (Phase B), with a clear "how the oracle drives it" section.
- **Top-level README**: link the SDKs, the escrow template, and the integration
  example; link the e-commerce repo as the live example (A2/D2).

---

## Testing & Evidence

### B2 — Escrow behaves for delivery and timeout scenarios

- **Existing** e-commerce coverage already proves the escrow primitives:
  `escrow_happy_release.e2e.ts`, `escrow_refund_timeout.e2e.ts`,
  `escrow_release_before_grace_fails`, `escrow_double_mark_shipped_fails`,
  `escrow_refund_after_shipped_fails` + Aiken validator tests.
- **New keeper-driven e2e tests** (the milestone's specific evidence):
  1. **Delivery → release**: lock escrow → oracle (mock/Shippo test tracking)
     reports `IN_TRANSIT` → keeper submits `mark_shipped` (escrow `Shipped`,
     refund now blocked) → oracle reports `DELIVERED` → keeper submits `release`
     once `now >= grace_period_end`; assert DB `released` + on-chain UTxO consumed
     to merchant.
  2. **Refund-blocked-after-dispatch**: lock escrow → oracle `IN_TRANSIT` →
     `mark_shipped` → attempt buyer refund → assert it **fails** (escrow no longer
     `Pending`). Proves the merchant protection. (Complements the existing
     `escrow_refund_after_shipped_fails`.)
  3. **Timeout → refund (buyer-initiated)**: lock escrow → oracle never confirms
     dispatch (`PRE_TRANSIT`/no movement) → advance past `ship_deadline` → buyer
     runs `escrow-refund.ts` → assert DB `refunded` + funds back to buyer. Reuses
     the existing `escrow_refund_timeout.e2e.ts` (buyer signs); the keeper is not
     involved — referenced here as the B2 timeout evidence, not re-implemented.
  4. **No-tracking fallback**: order without tracking → keeper does not touch the
     oracle and does not auto-settle (manual flow intact).
  - Run against **local dolos devnet** with short windows
    (`ESCROW_SHIP_DEADLINE_SECONDS` / `ESCROW_GRACE_PERIOD_SECONDS` set to
    seconds, as the existing e2e tests do). Capture pass/fail output as the B2
    report.
- **TS SDK verify() parity tests** (Phase A.1): the three pinned CBOR vectors.

> **C2 (video walkthrough):** handled outside this spec — no implementation
> tasks here. The end-to-end flow it would record (oracle `DELIVERED` →
> auto-release; timeout → auto-refund) is fully covered as runnable code by the
> keeper-driven e2e tests above, on local dolos.

### A2 / D2 — Links

Public repos: `shipping-oracle` (SDKs, docs, escrow template doc, integration
guide) and `e-commerce` (deployed escrow + keeper). Both linked from the
top-level README and in the milestone evidence.

---

## Implementation Order

1. **TS SDK `verify()` parity harness FIRST** — port the three pinned CBOR
   vectors and prove TS Ed25519 + CBOR + blake2b match Rust/Aiken. Blocks the
   keeper (a wrong `verify()` makes the whole integration untrustworthy).
2. `sdk/typescript/` — `OracleClient.fetchAttestation` + `OracleAttestation.verify` + types.
3. `sdk/typescript/README.md` + Rust SDK README parity + order-link example.
4. e-commerce: wire the TS SDK as a git/path dependency.
5. e-commerce keeper service module (`src/lib/cardano/…`) reusing
   `mark_shipped` + `submitReleaseEscrow` (release path only; refund stays
   buyer-initiated per decision 5).
6. e-commerce `scripts/settle-escrows.ts` (poll-once CLI).
7. DB/label adjustments for the `delivered` semantic (migration only if needed).
8. Keeper-driven e2e tests (delivery / timeout / no-tracking) on local dolos.
9. `diagrams/milestone-3-*.puml` (+ build PNGs).
10. Documentation: integration guide, escrow template doc, trust-model +
    auto-refund-limitation sections, top-level README links.

## Key Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| TS `verify()` diverges from Rust/Aiken (CBOR/Ed25519/blake2b) | HIGH — silent signature disagreement | Port the three pinned CBOR vectors as TS tests *first*; reuse the e-commerce CBOR encoder (same `Constr(121, indefinite)` shape) |
| Keeper non-idempotency / double-settle | MED — funds moved twice / DB drift | Gate every action on current escrow `status` + time window; chain-submit before DB write (existing optimistic pattern) |
| Reviewer expects on-chain oracle enforcement | MED — perceived as "not trustless" | Document the trust model honestly; note the on-chain-enforced escrow as a non-blocked follow-up |
| `shipped` vs `delivered` naming confusion | LOW | Relabel in DB/UI/docs; on-chain name documented as implementation detail |
| Cross-repo coupling (SDK consumed by e-commerce) | LOW | git/path dependency for the demo; npm publish noted as follow-up |

## Files Summary

| File | Action | Repo |
|------|--------|------|
| `sdk/typescript/` (package, src, README, tests) | NEW | shipping-oracle |
| `sdk/rust/README.md` + order-link example | UPDATE | shipping-oracle |
| `docs/` integration guide + escrow template doc | NEW | shipping-oracle |
| `README.md` (link SDKs / template / example) | UPDATE | shipping-oracle |
| `diagrams/milestone-3-*.puml` + PNGs | NEW | shipping-oracle |
| `spec/002-milestone-3.md` (this file) | NEW | shipping-oracle |
| `src/lib/cardano/` keeper service module | NEW | e-commerce |
| `scripts/settle-escrows.ts` | NEW | e-commerce |
| TS SDK as git/path dependency | NEW (package.json) | e-commerce |
| Supabase migration for `delivered` label | NEW (if needed) | e-commerce |
| `tests/e2e/escrow_oracle_*.e2e.ts` (delivery / refund-blocked-after-dispatch / no-tracking) | NEW | e-commerce |
| `tests/e2e/escrow_refund_timeout.e2e.ts` (timeout, buyer-initiated) | REUSE (B2 evidence) | e-commerce |
| `aiken/`, `tx3/`, existing `scripts/escrow-*.ts` | REUSE (no change) | e-commerce |

---

## Out of Scope (explicit)

- On-chain oracle enforcement in the escrow (withdrawal trick inside the escrow
  validator). Clean follow-up; blocked by `pallas-validate` on local dolos and
  not required for this milestone.
- npm publishing of the SDKs (git/path dependency suffices for the demo).
- A full daemon/long-running keeper (poll-once cron-style is enough).
- Keeper-automated refund (refund stays buyer-initiated; a permissionless-refund
  validator change would be the path to automate it, and is itself out of scope).
