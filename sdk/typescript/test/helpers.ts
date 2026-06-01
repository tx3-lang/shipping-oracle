/**
 * Test helpers for verifyAttestation tests.
 *
 * Builds fully-valid self-signed attestations using a deterministic private key.
 */

import { ed25519 } from "@noble/curves/ed25519";
import { blake2b } from "@noble/hashes/blake2b";
import { bytesToHex } from "@noble/hashes/utils";
import { encodeOracleDataCbor } from "../src/cbor.js";
import type { OracleAttestation, OracleStatus } from "../src/types.js";

// Deterministic 32-byte private key: all bytes = 1.
const PRIV_KEY = new Uint8Array(32).fill(1);

export interface BuildOptions {
  carrier?: string;
  tracking?: string;
  status?: OracleStatus;
  timestamp?: number;
}

/**
 * Build a fully valid self-signed attestation.
 * Uses a deterministic private key (all 0x01) so tests are reproducible.
 */
export function buildSignedAttestation(overrides?: BuildOptions): OracleAttestation {
  const carrier = overrides?.carrier ?? "shippo";
  const tracking = overrides?.tracking ?? "SHIPPO_DELIVERED";
  const status: OracleStatus = overrides?.status ?? "DELIVERED";
  const timestamp = overrides?.timestamp ?? 1712000000;

  const pub = ed25519.getPublicKey(PRIV_KEY);

  const encoder = new TextEncoder();
  const carrierHash = blake2b(encoder.encode(carrier), { dkLen: 32 });
  const trackingHash = blake2b(encoder.encode(tracking), { dkLen: 32 });

  const data = {
    carrier_hash: bytesToHex(carrierHash),
    tracking_number_hash: bytesToHex(trackingHash),
    status,
    timestamp,
  };

  const cbor = encodeOracleDataCbor(data);
  const sig = ed25519.sign(cbor, PRIV_KEY);

  return {
    data,
    plaintext: { carrier, tracking_number: tracking },
    signature: bytesToHex(sig),
    public_key: bytesToHex(pub),
    cbor_hex: bytesToHex(cbor),
  };
}

/**
 * The expected public key hex for the deterministic private key (all 0x01).
 */
export function expectedPublicKeyHex(): string {
  return bytesToHex(ed25519.getPublicKey(PRIV_KEY));
}
