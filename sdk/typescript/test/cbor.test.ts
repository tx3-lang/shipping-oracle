import { describe, it, expect } from "vitest";
import { bytesToHex } from "@noble/hashes/utils";
import { encodeOracleDataCbor } from "../src/cbor.js";
import type { OracleData } from "../src/types.js";

// ── Pinned CBOR vectors ────────────────────────────────────────────────────
// These exact byte sequences are produced by Aiken `builtin.serialise_data`
// (see onchain/lib/cbor_alignment_tests.ak) and locked by Rust minicbor
// (see backend/tests/cbor_alignment.rs).  A single wrong byte causes silent
// signature-verification failure on-chain.  Do NOT weaken these assertions.

describe("encodeOracleDataCbor — pinned CBOR vectors", () => {
  it("DELIVERED vector", () => {
    const data: OracleData = {
      carrier_hash: "11".repeat(32),
      tracking_number_hash: "22".repeat(32),
      status: "DELIVERED",
      timestamp: 1712000000,
    };

    const result = bytesToHex(encodeOracleDataCbor(data));

    // d879 9f  5820 <32×11>  5820 <32×22>  4944454c495645524544  1a660b0c00  ff
    const expected =
      "d8799f" +
      "5820" + "11".repeat(32) +
      "5820" + "22".repeat(32) +
      "4944454c4956455245441a660b0c00ff";

    expect(result).toBe(expected);
  });

  it("UNKNOWN vector", () => {
    const data: OracleData = {
      carrier_hash: "33".repeat(32),
      tracking_number_hash: "44".repeat(32),
      status: "UNKNOWN",
      timestamp: 1712345678,
    };

    const result = bytesToHex(encodeOracleDataCbor(data));

    // d879 9f  5820 <32×33>  5820 <32×44>  47554e4b4e4f574e  1a6610524e  ff
    const expected =
      "d8799f" +
      "5820" + "33".repeat(32) +
      "5820" + "44".repeat(32) +
      "47554e4b4e4f574e1a6610524eff";

    expect(result).toBe(expected);
  });

  it("zero-edge vector (empty hashes, empty status, timestamp=0)", () => {
    // OracleStatus doesn't include "", so we cast for this boundary test.
    const data = {
      carrier_hash: "",
      tracking_number_hash: "",
      status: "" as OracleData["status"],
      timestamp: 0,
    };

    const result = bytesToHex(encodeOracleDataCbor(data));

    // d879 9f  40  40  40  00  ff
    expect(result).toBe("d8799f40404000ff");
  });
});
