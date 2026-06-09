import { describe, it, expect } from "vitest";
import { toConsumeOracleDataArgs } from "../src/tx3.js";
import { OracleSdkError } from "../src/error.js";
import { buildSignedAttestation } from "./helpers.js";

describe("toConsumeOracleDataArgs", () => {
  it("maps an attestation to the tx3 consume_oracle_data args", () => {
    const att = buildSignedAttestation({
      status: "IN_TRANSIT",
      timestamp: 1777493042,
    });

    const args = toConsumeOracleDataArgs(att);

    expect(args.p_carrier_hash).toBe(att.data.carrier_hash);
    expect(args.p_tracking_number_hash).toBe(att.data.tracking_number_hash);
    expect(args.p_timestamp).toBe(1777493042);
    expect(args.p_signature).toBe(att.signature);
  });

  it("encodes p_status as the hex of the UTF-8 status bytes (parity with Rust/tx3)", () => {
    // "IN_TRANSIT" → 494e5f5452414e534954, matching tx3/local_consume_oracle.json
    const att = buildSignedAttestation({ status: "IN_TRANSIT" });
    expect(toConsumeOracleDataArgs(att).p_status).toBe("494e5f5452414e534954");

    // "DELIVERED" → 44454c495645524544, matching the Rust SDK test vector
    const att2 = buildSignedAttestation({ status: "DELIVERED" });
    expect(toConsumeOracleDataArgs(att2).p_status).toBe("44454c495645524544");
  });

  it("throws INVALID_LENGTH when a hash is not 32 bytes", () => {
    const att = buildSignedAttestation();
    att.data.carrier_hash = "abcd"; // 2 bytes, not 32
    let error: unknown;
    try {
      toConsumeOracleDataArgs(att);
    } catch (e) {
      error = e;
    }
    expect(error).toBeInstanceOf(OracleSdkError);
    expect((error as OracleSdkError).code).toBe("INVALID_LENGTH");
  });

  it("throws INVALID_LENGTH when the signature is not 64 bytes", () => {
    const att = buildSignedAttestation();
    att.signature = "00".repeat(32); // 32 bytes, not 64
    let error: unknown;
    try {
      toConsumeOracleDataArgs(att);
    } catch (e) {
      error = e;
    }
    expect(error).toBeInstanceOf(OracleSdkError);
    expect((error as OracleSdkError).code).toBe("INVALID_LENGTH");
  });
});
