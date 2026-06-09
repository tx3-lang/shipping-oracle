/**
 * tx3 argument mapping for the Shipping Oracle SDK.
 *
 * Ports sdk/rust/src/tx3.rs: turns a verified `OracleAttestation` into the
 * argument object the tx3 `consume_oracle_data` transaction expects. The
 * mapping is byte-identical to the Rust SDK — notably `p_status` is the hex of
 * the UTF-8 status bytes, not the status string itself.
 *
 * This only produces the args; transaction assembly is handled by tx3 or the
 * consuming dApp. Verify the attestation first (`verifyAttestation` /
 * `prepareCommitment`) before submitting these args on-chain.
 */

import { bytesToHex } from "@noble/hashes/utils";
import { decodeFixed } from "./verify.js";
import type { OracleAttestation } from "./types.js";

/**
 * Arguments for the tx3 `consume_oracle_data` transaction.
 * Field names match the tx3 protocol params exactly; all byte fields are
 * lowercase hex.
 */
export interface ConsumeOracleDataArgs {
  /** blake2b-256 carrier hash, 32 bytes, lowercase hex */
  p_carrier_hash: string;
  /** blake2b-256 tracking-number hash, 32 bytes, lowercase hex */
  p_tracking_number_hash: string;
  /** UTF-8 bytes of the status string, lowercase hex (e.g. "IN_TRANSIT" → 494e5f5452414e534954) */
  p_status: string;
  /** Unix timestamp in seconds (i64) */
  p_timestamp: number;
  /** Ed25519 signature, 64 bytes, lowercase hex */
  p_signature: string;
}

/**
 * Map an attestation to the tx3 `consume_oracle_data` arguments.
 *
 * Throws `OracleSdkError('INVALID_LENGTH', ...)` if a hash is not 32 bytes or
 * the signature is not 64 bytes.
 */
export function toConsumeOracleDataArgs(
  attestation: OracleAttestation
): ConsumeOracleDataArgs {
  const carrierHash = decodeFixed(
    "data.carrier_hash",
    attestation.data.carrier_hash,
    32
  );
  const trackingHash = decodeFixed(
    "data.tracking_number_hash",
    attestation.data.tracking_number_hash,
    32
  );
  const signature = decodeFixed("signature", attestation.signature, 64);

  return {
    p_carrier_hash: bytesToHex(carrierHash),
    p_tracking_number_hash: bytesToHex(trackingHash),
    p_status: bytesToHex(new TextEncoder().encode(attestation.data.status)),
    p_timestamp: attestation.data.timestamp,
    p_signature: bytesToHex(signature),
  };
}
