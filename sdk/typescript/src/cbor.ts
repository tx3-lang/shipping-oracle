/**
 * Canonical CBOR encoder for OracleData.
 *
 * Produces byte-for-byte identical output to:
 *   - Rust: pallas `minicbor::to_vec` of `PlutusData::Constr { tag: 121, fields: Indef([...]) }`
 *   - Aiken: `builtin.serialise_data(OracleData)`
 *
 * Hand-rolled — no CBOR library dependency.  Encoding rules:
 *   - Constr tag 121: 0xd8 0x79
 *   - Indefinite-length array: 0x9f … 0xff
 *   - Byte string (major 2): shortest-form header, then raw bytes
 *   - Unsigned integer (major 0): shortest-form (inline / 1B / 2B / 4B / 8B)
 *   - Negative integer (major 1): over (-1 - n), same length rules
 *
 * Pinned test vectors live in:
 *   sdk/typescript/test/cbor.test.ts
 *   backend/tests/cbor_alignment.rs
 *   onchain/lib/cbor_alignment_tests.ak
 */

import { hexToBytes } from "@noble/hashes/utils";
import type { OracleData } from "./types.js";

// ── Low-level CBOR helpers ────────────────────────────────────────────────

/**
 * Encode a CBOR "additional info" header for the given major type and value.
 * Returns the minimal-length encoding as a Uint8Array.
 *
 * @param major  CBOR major type (0–7)
 * @param value  The numeric value to encode (must fit in a safe 53-bit JS number
 *               for the 8-byte path we use BigInt internally to avoid sign issues)
 */
function encodeHeader(major: number, value: number): Uint8Array {
  const mt = (major & 0x07) << 5;
  if (value < 24) {
    return new Uint8Array([mt | value]);
  }
  if (value <= 0xff) {
    return new Uint8Array([mt | 24, value]);
  }
  if (value <= 0xffff) {
    return new Uint8Array([mt | 25, (value >> 8) & 0xff, value & 0xff]);
  }
  if (value <= 0xffffffff) {
    return new Uint8Array([
      mt | 26,
      (value >>> 24) & 0xff,
      (value >>> 16) & 0xff,
      (value >>> 8) & 0xff,
      value & 0xff,
    ]);
  }
  // 8-byte path: use BigInt to avoid 32-bit sign issues
  const big = BigInt(value);
  return new Uint8Array([
    mt | 27,
    Number((big >> 56n) & 0xffn),
    Number((big >> 48n) & 0xffn),
    Number((big >> 40n) & 0xffn),
    Number((big >> 32n) & 0xffn),
    Number((big >> 24n) & 0xffn),
    Number((big >> 16n) & 0xffn),
    Number((big >> 8n) & 0xffn),
    Number(big & 0xffn),
  ]);
}

/** Encode a CBOR byte string (major type 2). */
function encodeBytes(bytes: Uint8Array): Uint8Array {
  const header = encodeHeader(2, bytes.length);
  const out = new Uint8Array(header.length + bytes.length);
  out.set(header, 0);
  out.set(bytes, header.length);
  return out;
}

/** Encode a CBOR unsigned/negative integer (major 0 or 1), shortest form. */
function encodeInt(n: number): Uint8Array {
  if (n >= 0) {
    return encodeHeader(0, n);
  }
  // Negative: major type 1, value = -1 - n  (i.e. ~n)
  return encodeHeader(1, -1 - n);
}

/** Concatenate an array of Uint8Arrays into one. */
function concat(parts: Uint8Array[]): Uint8Array {
  let total = 0;
  for (const p of parts) total += p.length;
  const out = new Uint8Array(total);
  let offset = 0;
  for (const p of parts) {
    out.set(p, offset);
    offset += p.length;
  }
  return out;
}

// ── Public encoder ────────────────────────────────────────────────────────

/**
 * Encode `OracleData` as canonical PlutusData CBOR.
 *
 * Structure: `Constr(121, [bytes(carrier_hash), bytes(tracking_number_hash),
 *                          bytes(status_utf8), int(timestamp)])`
 *
 * Uses an indefinite-length array for the constructor fields, matching the
 * encoding produced by Aiken's `builtin.serialise_data` and pallas minicbor.
 */
export function encodeOracleDataCbor(data: OracleData): Uint8Array {
  // Constr tag 121 encodes as CBOR tag 121.
  // CBOR tag with 1-byte additional info: 0xd8 <tag>
  // 121 = 0x79
  const constrTag = new Uint8Array([0xd8, 0x79]);

  // Indefinite-length array: 0x9f … 0xff
  const arrayOpen = new Uint8Array([0x9f]);
  const arrayBreak = new Uint8Array([0xff]);

  // Field 1: carrier_hash (hex → bytes)
  const carrierBytes =
    data.carrier_hash.length > 0 ? hexToBytes(data.carrier_hash) : new Uint8Array(0);

  // Field 2: tracking_number_hash (hex → bytes)
  const trackingBytes =
    data.tracking_number_hash.length > 0
      ? hexToBytes(data.tracking_number_hash)
      : new Uint8Array(0);

  // Field 3: status (UTF-8 encoded)
  const statusBytes = new TextEncoder().encode(data.status);

  // Field 4: timestamp (integer)
  const timestampEncoded = encodeInt(data.timestamp);

  return concat([
    constrTag,
    arrayOpen,
    encodeBytes(carrierBytes),
    encodeBytes(trackingBytes),
    encodeBytes(statusBytes),
    timestampEncoded,
    arrayBreak,
  ]);
}
