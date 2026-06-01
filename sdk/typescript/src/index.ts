/**
 * Public entrypoint for the Shipping Oracle TypeScript SDK.
 *
 * Re-exports the full public API: client, verification, CBOR encoder, error
 * types, and wire types.
 */

// ── Client ────────────────────────────────────────────────────────────────────
export { OracleClient } from "./client.js";
export type { OracleClientOptions } from "./client.js";

// ── Verification ──────────────────────────────────────────────────────────────
export { verifyAttestation } from "./verify.js";
export type { VerifyOptions } from "./verify.js";

// ── CBOR encoder ─────────────────────────────────────────────────────────────
export { encodeOracleDataCbor } from "./cbor.js";

// ── Error ─────────────────────────────────────────────────────────────────────
export { OracleSdkError } from "./error.js";
export type { OracleSdkErrorCode } from "./error.js";

// ── Wire types ────────────────────────────────────────────────────────────────
export type {
  OracleStatus,
  OracleData,
  ShipmentPlaintext,
  OracleAttestation,
  HealthResponse,
  PreparedCommitment,
} from "./types.js";
