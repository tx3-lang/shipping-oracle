# Shipping Oracle TypeScript SDK

TypeScript SDK for applications that need to consume the Shipping Oracle over HTTP, verify the returned attestation, and prepare it for on-chain use with the `consume_oracle_data` flow.

## What It Covers

- Fetch `GET /v1/shipment` attestations from the oracle backend.
- Verify Ed25519 signatures over the canonical CBOR payload — byte-identical to the Rust SDK and the Aiken on-chain validator.
- Check that the signed `OracleData` matches the returned plaintext shipment identifiers.
- Link application events or orders to a verified on-chain shipment tracking commitment.
- Keep application context (e.g. `orderId`, UTxO refs) attached to the resulting commitment package.

This SDK is the Milestone 3 deliverable for acceptance criterion **A1**: developers can link application events to on-chain shipment tracking commitments without hand-assembling `curl`, `jq`, and hex-conversion steps.

## Install

While the SDK is still in-repo, add it as a path dependency:

```json
{
  "dependencies": {
    "shipping-oracle-sdk": "file:../sdk/typescript"
  }
}
```

Publishing to the npm registry (`pnpm publish`) is a future step.

## Quick Start

```typescript
import { OracleClient } from "shipping-oracle-sdk";

interface OrderContext {
  orderId: string;
}

const client = new OracleClient("http://127.0.0.1:3000", {
  // Optional: pin the expected oracle public key.
  // expectedPublicKeyHex: process.env.ORACLE_PUBLIC_KEY,
});

const commitment = await client.prepareCommitment<OrderContext>(
  { orderId: "ord_123" },
  "shippo",
  "SHIPPO_DELIVERED"
);

console.log("linked order:", commitment.context.orderId);
console.log("status      :", commitment.attestation.data.status);
console.log("carrier_hash:", commitment.attestation.data.carrier_hash);
console.log("cbor_hex    :", commitment.attestation.cbor_hex);
```

## Public API

### `OracleClient`

```typescript
new OracleClient(baseUrl: string, options?: OracleClientOptions)
```

- `baseUrl` — Base URL of the oracle HTTP API (e.g. `http://127.0.0.1:3000`).
- `options.expectedPublicKeyHex` — When set, `prepareCommitment` asserts that the
  attestation was signed by this key (hex-encoded Ed25519 public key, 32 bytes).
- `options.fetchFn` — Override the global `fetch`. Useful for testing.

**Methods:**

- `client.health()` — `GET /health`. Returns `HealthResponse`.
- `client.fetchAttestation(carrier, trackingNumber)` — `GET /v1/shipment`.
  Returns the raw `OracleAttestation` (no cryptographic verification).
- `client.prepareCommitment<TContext>(context, carrier, trackingNumber)` —
  Fetches the attestation, asserts the response matches the requested shipment,
  runs full cryptographic verification, and returns `PreparedCommitment<TContext>`.

### `verifyAttestation(attestation, options?)`

Standalone verification function. Throws `OracleSdkError` on any failure.

Checks:
- The Ed25519 `signature` validates under `public_key`.
- The canonical CBOR in `cbor_hex` matches the declared `data` fields (carrier
  hash, tracking number hash, status, timestamp).
- `plaintext.carrier` hashes to `data.carrier_hash` (blake2b-256).
- `plaintext.tracking_number` hashes to `data.tracking_number_hash` (blake2b-256).
- If `options.expectedPublicKeyHex` is set, `public_key` must match exactly.

### `encodeOracleDataCbor(data: OracleData): Uint8Array`

Encodes `OracleData` to canonical CBOR — `Constr 121` with an indefinite-length
array (`d879 9f ... ff`). Produces byte-identical output to Aiken
`builtin.serialise_data` and the Rust `minicbor` encoder.

### Types

| Type | Description |
|---|---|
| `OracleAttestation` | Full oracle response (data, plaintext, signature, public_key, cbor_hex) |
| `OracleData` | The signed payload: carrier_hash, tracking_number_hash, status, timestamp |
| `ShipmentPlaintext` | Unverified plaintext: carrier, tracking_number |
| `PreparedCommitment<TContext>` | Verified attestation + your application context |
| `OracleStatus` | `"DELIVERED" \| "NOT_DELIVERED" \| "IN_TRANSIT" \| "PRE_TRANSIT" \| "UNKNOWN"` |
| `HealthResponse` | `{ status: string }` |
| `OracleSdkError` | SDK error (extends `Error`; `code: OracleSdkErrorCode`) |

### `OracleAttestation` shape

```json
{
  "data": {
    "carrier_hash": "<blake2b-256 hex of carrier>",
    "tracking_number_hash": "<blake2b-256 hex of tracking number>",
    "status": "DELIVERED",
    "timestamp": 1712000000
  },
  "plaintext": {
    "carrier": "shippo",
    "tracking_number": "SHIPPO_DELIVERED"
  },
  "signature": "<Ed25519 signature, 64 bytes hex>",
  "public_key": "<Ed25519 public key, 32 bytes hex>",
  "cbor_hex": "<canonical CBOR of data, hex>"
}
```

> **Caution: never re-serialize `cbor_hex`.** The Ed25519 signature covers the
> exact bytes encoded in `cbor_hex`. Re-encoding or normalizing that field — even
> to semantically equivalent CBOR — will produce a different byte sequence and
> cause on-chain signature verification to fail silently. Always embed `cbor_hex`
> verbatim in the transaction redeemer.

## Verification and CBOR Parity

Verification reproduces:

1. **Ed25519** — signature check using `@noble/curves`.
2. **Canonical CBOR** — `Constr 121` tag (`0xd879`), indefinite-length array
   (`0x9f ... 0xff`), with blake2b-256 hash fields encoded as 32-byte CBOR
   byte strings and `status` as a CBOR byte string of its UTF-8 bytes.
3. **blake2b-256** — hash of plaintext `carrier` and `tracking_number`, verified
   against the signed `carrier_hash` and `tracking_number_hash` fields.

The CBOR parity tests in `test/cbor.test.ts` lock three pinned byte vectors
(DELIVERED, UNKNOWN, and the zero-edge case) that are byte-identical across:

- `onchain/lib/cbor_alignment_tests.ak` — Aiken `builtin.serialise_data`
- `backend/tests/cbor_alignment.rs` — Rust `minicbor` / pallas
- `sdk/typescript/test/cbor.test.ts` — this SDK's `encodeOracleDataCbor`

A single wrong byte in any of these three causes silent signature-verification
failure on-chain. Do not weaken or remove those assertions.

## Examples

Run the example against a local backend:

```bash
cd sdk/typescript
pnpx tsx examples/order-commitment.ts
```

Optional environment variables:

- `ORACLE_BASE_URL` defaults to `http://127.0.0.1:3000`
- `ORACLE_PUBLIC_KEY` pins the expected oracle verification key (hex)

## Tests

```bash
cd sdk/typescript
pnpm install
pnpm test
```

The test suite covers:

- CBOR encoding parity with the Aiken on-chain validator and Rust backend
  (three pinned byte vectors).
- `verifyAttestation` — valid signatures pass; tampered signature/cbor/hash/key
  all throw `OracleSdkError` with the correct code.
- `OracleClient` — `prepareCommitment` happy-path; carrier/tracking-number
  mismatch; non-2xx HTTP error; optional public-key pinning.
