/**
 * Wire types for the Shipping Oracle SDK.
 * Field names are snake_case to match the HTTP JSON wire format exactly —
 * never reshape before verification.
 */

/** Status codes returned by the oracle, mirroring the Rust OracleStatus enum. */
export type OracleStatus =
  | "DELIVERED"
  | "NOT_DELIVERED"
  | "IN_TRANSIT"
  | "PRE_TRANSIT"
  | "UNKNOWN";

/** Core oracle data that is CBOR-encoded and signed. */
export interface OracleData {
  /** blake2b-256 hex of the carrier name */
  carrier_hash: string;
  /** blake2b-256 hex of the tracking number */
  tracking_number_hash: string;
  /** Current shipment status */
  status: OracleStatus;
  /** Unix timestamp in seconds (i64) */
  timestamp: number;
}

/** Plaintext shipment identifiers (not included in the signed payload). */
export interface ShipmentPlaintext {
  carrier: string;
  tracking_number: string;
}

/**
 * Full oracle attestation as returned by the HTTP API.
 * `cbor_hex` is the canonical CBOR encoding of `data` (Constr 121, indefinite),
 * and `signature` covers exactly those bytes.
 */
export interface OracleAttestation {
  data: OracleData;
  plaintext: ShipmentPlaintext;
  /** Ed25519 signature over cbor_hex bytes — 64 bytes, hex-encoded */
  signature: string;
  /** Ed25519 public key — 32 bytes, hex-encoded */
  public_key: string;
  /** Canonical CBOR of `data` (hex). Embed this verbatim in the on-chain redeemer. */
  cbor_hex: string;
}

/** Response from the /health endpoint. */
export interface HealthResponse {
  status: string;
}

/**
 * A prepared commitment ready for on-chain submission.
 * `TContext` is caller-defined context (e.g. UTxO refs, wallet info).
 */
export interface PreparedCommitment<TContext> {
  context: TContext;
  attestation: OracleAttestation;
}
