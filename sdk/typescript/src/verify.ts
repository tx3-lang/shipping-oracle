/**
 * Attestation verification for the Shipping Oracle SDK.
 *
 * Ports sdk/rust/src/verify.rs exactly.  The verification order is:
 *   1. Decode & optionally pin public key (32 bytes)
 *   2. Decode signature (64 bytes), Ed25519-verify over raw cbor_hex bytes
 *   3. Re-encode data → must match cbor_hex bytes
 *   4. blake2b-256(utf8(carrier))   == data.carrier_hash
 *      blake2b-256(utf8(tracking))  == data.tracking_number_hash
 */

import { ed25519 } from "@noble/curves/ed25519";
import { blake2b } from "@noble/hashes/blake2b";
import { hexToBytes, bytesToHex } from "@noble/hashes/utils";
import { encodeOracleDataCbor } from "./cbor.js";
import { OracleSdkError } from "./error.js";
import type { OracleAttestation } from "./types.js";

export interface VerifyOptions {
  /** If set, the attestation's public_key must equal this hex string exactly. */
  expectedPublicKeyHex?: string;
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/** Decode a hex string and assert the result is exactly N bytes. */
function decodeFixed(
  fieldName: string,
  hexStr: string,
  expectedLen: number
): Uint8Array {
  let bytes: Uint8Array;
  try {
    bytes = hexToBytes(hexStr);
  } catch {
    throw new OracleSdkError(
      "INVALID_LENGTH",
      `${fieldName}: invalid hex encoding`
    );
  }
  if (bytes.length !== expectedLen) {
    throw new OracleSdkError(
      "INVALID_LENGTH",
      `${fieldName}: expected ${expectedLen} bytes, got ${bytes.length}`
    );
  }
  return bytes;
}

/** Blake2b-256 of a UTF-8 encoded string, returned as lowercase hex. */
function blake2b256Hex(input: string): string {
  const encoder = new TextEncoder();
  const hash = blake2b(encoder.encode(input), { dkLen: 32 });
  return bytesToHex(hash);
}

// ── Public API ────────────────────────────────────────────────────────────────

/**
 * Verify an oracle attestation.
 *
 * Returns `void` on success.  Throws `OracleSdkError` on any failure:
 *   - `INVALID_LENGTH`              — bad hex length for public_key or signature
 *   - `UNEXPECTED_PUBLIC_KEY`       — public key does not match expectedPublicKeyHex
 *   - `INVALID_SIGNATURE`           — Ed25519 verification failed
 *   - `CBOR_MISMATCH`               — re-encoded data ≠ cbor_hex
 *   - `CARRIER_HASH_MISMATCH`       — blake2b-256(carrier) ≠ data.carrier_hash
 *   - `TRACKING_NUMBER_HASH_MISMATCH` — blake2b-256(tracking_number) ≠ data.tracking_number_hash
 */
export function verifyAttestation(
  attestation: OracleAttestation,
  options?: VerifyOptions
): void {
  // ── Step 1: Decode & optionally pin public key ─────────────────────────────
  const publicKeyBytes = decodeFixed("public_key", attestation.public_key, 32);

  if (options?.expectedPublicKeyHex !== undefined) {
    const expected = options.expectedPublicKeyHex.toLowerCase();
    const actual = attestation.public_key.toLowerCase();
    if (actual !== expected) {
      throw new OracleSdkError(
        "UNEXPECTED_PUBLIC_KEY",
        `public_key does not match expectedPublicKeyHex`
      );
    }
  }

  // ── Step 2: Decode signature & verify over raw cbor_hex bytes ─────────────
  const signatureBytes = decodeFixed("signature", attestation.signature, 64);

  let cborBytes: Uint8Array;
  try {
    cborBytes = hexToBytes(attestation.cbor_hex);
  } catch {
    throw new OracleSdkError("INVALID_SIGNATURE", "cbor_hex: invalid hex encoding");
  }

  try {
    // zip215: false → strict RFC8032 verification, matching ed25519-dalek
    // (Rust SDK) and the on-chain Plutus verifier. Avoids accepting any
    // ZIP215-only-valid signature the chain would reject.
    const valid = ed25519.verify(signatureBytes, cborBytes, publicKeyBytes, {
      zip215: false,
    });
    if (!valid) {
      throw new OracleSdkError("INVALID_SIGNATURE", "Ed25519 signature verification failed");
    }
  } catch (e) {
    if (e instanceof OracleSdkError) throw e;
    throw new OracleSdkError("INVALID_SIGNATURE", "Ed25519 signature verification failed");
  }

  // ── Step 3: Re-encode data and compare to cbor_hex ────────────────────────
  // encodeOracleDataCbor → hexToBytes throws a raw Error on malformed hash hex;
  // surface it as OracleSdkError so every failure mode stays typed.
  let reencoded: Uint8Array;
  try {
    reencoded = encodeOracleDataCbor(attestation.data);
  } catch {
    throw new OracleSdkError(
      "CBOR_MISMATCH",
      "data fields could not be re-encoded (invalid hash hex?)"
    );
  }
  if (reencoded.length !== cborBytes.length) {
    throw new OracleSdkError("CBOR_MISMATCH", "Re-encoded CBOR does not match cbor_hex");
  }
  for (let i = 0; i < reencoded.length; i++) {
    if (reencoded[i] !== cborBytes[i]) {
      throw new OracleSdkError("CBOR_MISMATCH", "Re-encoded CBOR does not match cbor_hex");
    }
  }

  // ── Step 4: Verify carrier and tracking_number hashes ─────────────────────
  const carrierHashActual = blake2b256Hex(attestation.plaintext.carrier);
  if (carrierHashActual !== attestation.data.carrier_hash.toLowerCase()) {
    throw new OracleSdkError(
      "CARRIER_HASH_MISMATCH",
      "blake2b-256(plaintext.carrier) does not match data.carrier_hash"
    );
  }

  const trackingHashActual = blake2b256Hex(attestation.plaintext.tracking_number);
  if (trackingHashActual !== attestation.data.tracking_number_hash.toLowerCase()) {
    throw new OracleSdkError(
      "TRACKING_NUMBER_HASH_MISMATCH",
      "blake2b-256(plaintext.tracking_number) does not match data.tracking_number_hash"
    );
  }
}
