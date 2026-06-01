import { describe, it, expect } from "vitest";
import { bytesToHex, hexToBytes } from "@noble/hashes/utils";
import { verifyAttestation } from "../src/verify.js";
import { OracleSdkError } from "../src/error.js";
import { buildSignedAttestation, expectedPublicKeyHex } from "./helpers.js";

// ── Helper: flip one byte of a hex string at position byteIndex ──────────────
function flipByte(hex: string, byteIndex: number): string {
  const bytes = hexToBytes(hex);
  bytes[byteIndex] ^= 0xff;
  return bytesToHex(bytes);
}

describe("verifyAttestation", () => {
  it("passes for a valid attestation", () => {
    const att = buildSignedAttestation();
    // Must not throw
    expect(() => verifyAttestation(att)).not.toThrow();
  });

  it("passes with a correct pinned public key", () => {
    const att = buildSignedAttestation();
    expect(() =>
      verifyAttestation(att, { expectedPublicKeyHex: expectedPublicKeyHex() })
    ).not.toThrow();
  });

  it("throws UNEXPECTED_PUBLIC_KEY when pinned key does not match", () => {
    const att = buildSignedAttestation();
    // Use a key that is the right length but different
    const wrongKey = "00".repeat(32);
    let error: unknown;
    try {
      verifyAttestation(att, { expectedPublicKeyHex: wrongKey });
    } catch (e) {
      error = e;
    }
    expect(error).toBeInstanceOf(OracleSdkError);
    expect((error as OracleSdkError).code).toBe("UNEXPECTED_PUBLIC_KEY");
  });

  it("throws INVALID_SIGNATURE when signature has a flipped byte", () => {
    const att = buildSignedAttestation();
    const corrupted = { ...att, signature: flipByte(att.signature, 0) };
    let error: unknown;
    try {
      verifyAttestation(corrupted);
    } catch (e) {
      error = e;
    }
    expect(error).toBeInstanceOf(OracleSdkError);
    expect((error as OracleSdkError).code).toBe("INVALID_SIGNATURE");
  });

  it("throws CBOR_MISMATCH when data.status is mutated after signing", () => {
    const att = buildSignedAttestation();
    // Mutate the status so re-encoding produces a different CBOR — but cbor_hex is still the old one
    const corrupted = {
      ...att,
      data: { ...att.data, status: "UNKNOWN" as const },
    };
    let error: unknown;
    try {
      verifyAttestation(corrupted);
    } catch (e) {
      error = e;
    }
    expect(error).toBeInstanceOf(OracleSdkError);
    expect((error as OracleSdkError).code).toBe("CBOR_MISMATCH");
  });

  it("throws CARRIER_HASH_MISMATCH when plaintext.carrier does not match data.carrier_hash", () => {
    const att = buildSignedAttestation();
    // Mutate the plaintext carrier — so blake2b(carrier) ≠ data.carrier_hash
    const corrupted = {
      ...att,
      plaintext: { ...att.plaintext, carrier: "wrong-carrier" },
    };
    let error: unknown;
    try {
      verifyAttestation(corrupted);
    } catch (e) {
      error = e;
    }
    expect(error).toBeInstanceOf(OracleSdkError);
    expect((error as OracleSdkError).code).toBe("CARRIER_HASH_MISMATCH");
  });

  it("throws TRACKING_NUMBER_HASH_MISMATCH when plaintext.tracking_number does not match", () => {
    const att = buildSignedAttestation();
    const corrupted = {
      ...att,
      plaintext: { ...att.plaintext, tracking_number: "WRONG_TRACKING" },
    };
    let error: unknown;
    try {
      verifyAttestation(corrupted);
    } catch (e) {
      error = e;
    }
    expect(error).toBeInstanceOf(OracleSdkError);
    expect((error as OracleSdkError).code).toBe("TRACKING_NUMBER_HASH_MISMATCH");
  });

  it("throws INVALID_LENGTH for a public key that is not 32 bytes", () => {
    const att = buildSignedAttestation();
    const corrupted = { ...att, public_key: "aabbcc" }; // only 3 bytes
    let error: unknown;
    try {
      verifyAttestation(corrupted);
    } catch (e) {
      error = e;
    }
    expect(error).toBeInstanceOf(OracleSdkError);
    expect((error as OracleSdkError).code).toBe("INVALID_LENGTH");
  });

  it("throws INVALID_LENGTH for a signature that is not 64 bytes", () => {
    const att = buildSignedAttestation();
    const corrupted = { ...att, signature: "aabbcc" }; // only 3 bytes
    let error: unknown;
    try {
      verifyAttestation(corrupted);
    } catch (e) {
      error = e;
    }
    expect(error).toBeInstanceOf(OracleSdkError);
    expect((error as OracleSdkError).code).toBe("INVALID_LENGTH");
  });
});
