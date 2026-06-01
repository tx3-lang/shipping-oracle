import { describe, it, expect } from "vitest";
import { OracleClient } from "../src/client.js";
import { OracleSdkError } from "../src/error.js";
import { buildSignedAttestation, expectedPublicKeyHex } from "./helpers.js";
import type { OracleAttestation } from "../src/types.js";

// ── Helpers ───────────────────────────────────────────────────────────────────

/** Build a minimal fetch function that returns a JSON response. */
function makeFetchFn(body: unknown, status = 200): typeof fetch {
  return async (_input: RequestInfo | URL, _init?: RequestInit) => {
    return new Response(JSON.stringify(body), {
      status,
      headers: { "content-type": "application/json" },
    });
  };
}

/** Build a fetch function that returns a plain text response (for error paths). */
function makeTextFetchFn(body: string, status: number): typeof fetch {
  return async (_input: RequestInfo | URL, _init?: RequestInit) => {
    return new Response(body, { status });
  };
}

// ── fetchAttestation ──────────────────────────────────────────────────────────

describe("OracleClient.fetchAttestation", () => {
  it("returns the parsed attestation", async () => {
    const att = buildSignedAttestation();
    const client = new OracleClient("https://oracle.example.com", {
      fetchFn: makeFetchFn(att),
    });

    const result = await client.fetchAttestation("shippo", "SHIPPO_DELIVERED");

    expect(result.data.status).toBe("DELIVERED");
    expect(result.plaintext.carrier).toBe("shippo");
    expect(result.plaintext.tracking_number).toBe("SHIPPO_DELIVERED");
  });

  it("URL-encodes query parameters", async () => {
    const att = buildSignedAttestation({
      carrier: "ups & co",
      tracking: "1Z 999 AA1",
    });

    let capturedUrl = "";
    const fetchFn: typeof fetch = async (input, _init?) => {
      capturedUrl = input.toString();
      return new Response(JSON.stringify(att), {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    };

    const client = new OracleClient("https://oracle.example.com", { fetchFn });
    await client.fetchAttestation("ups & co", "1Z 999 AA1");

    expect(capturedUrl).toContain("carrier=ups+%26+co");
    expect(capturedUrl).toContain("tracking_number=1Z+999+AA1");
  });

  it("trims trailing slash from baseUrl", async () => {
    const att = buildSignedAttestation();

    let capturedUrl = "";
    const fetchFn: typeof fetch = async (input, _init?) => {
      capturedUrl = input.toString();
      return new Response(JSON.stringify(att), {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    };

    const client = new OracleClient("https://oracle.example.com///", { fetchFn });
    await client.fetchAttestation("shippo", "SHIPPO_DELIVERED");

    expect(capturedUrl).toMatch(/^https:\/\/oracle\.example\.com\/v1\/shipment/);
    expect(capturedUrl).not.toMatch(/\/\/v1/);
  });

  it("throws OracleSdkError(API) on non-2xx response", async () => {
    const client = new OracleClient("https://oracle.example.com", {
      fetchFn: makeTextFetchFn("upstream boom", 502),
    });

    let error: unknown;
    try {
      await client.fetchAttestation("shippo", "SHIPPO_DELIVERED");
    } catch (e) {
      error = e;
    }

    expect(error).toBeInstanceOf(OracleSdkError);
    expect((error as OracleSdkError).code).toBe("API");
    expect((error as OracleSdkError).message).toContain("502");
    expect((error as OracleSdkError).message).toContain("upstream boom");
  });
});

// ── health ────────────────────────────────────────────────────────────────────

describe("OracleClient.health", () => {
  it("returns the health response", async () => {
    const client = new OracleClient("https://oracle.example.com", {
      fetchFn: makeFetchFn({ status: "ok" }),
    });

    const result = await client.health();
    expect(result.status).toBe("ok");
  });

  it("throws OracleSdkError(API) on non-2xx from /health", async () => {
    const client = new OracleClient("https://oracle.example.com", {
      fetchFn: makeTextFetchFn("service unavailable", 503),
    });

    let error: unknown;
    try {
      await client.health();
    } catch (e) {
      error = e;
    }

    expect(error).toBeInstanceOf(OracleSdkError);
    expect((error as OracleSdkError).code).toBe("API");
  });
});

// ── prepareCommitment ─────────────────────────────────────────────────────────

describe("OracleClient.prepareCommitment", () => {
  it("verifies and attaches context", async () => {
    const att = buildSignedAttestation();
    const context = { orderId: "order-123", utxo: "abc#0" };
    const client = new OracleClient("https://oracle.example.com", {
      fetchFn: makeFetchFn(att),
      expectedPublicKeyHex: expectedPublicKeyHex(),
    });

    const result = await client.prepareCommitment(
      context,
      "shippo",
      "SHIPPO_DELIVERED"
    );

    expect(result.context).toBe(context);
    expect(result.attestation.data.status).toBe("DELIVERED");
  });

  it("throws RESPONSE_MISMATCH when carrier in response does not match request", async () => {
    // Response carrier is "shippo", but we requested "dhl"
    const att = buildSignedAttestation({ carrier: "shippo", tracking: "SHIPPO_DELIVERED" });
    const client = new OracleClient("https://oracle.example.com", {
      fetchFn: makeFetchFn(att),
    });

    let error: unknown;
    try {
      await client.prepareCommitment({}, "dhl", "SHIPPO_DELIVERED");
    } catch (e) {
      error = e;
    }

    expect(error).toBeInstanceOf(OracleSdkError);
    expect((error as OracleSdkError).code).toBe("RESPONSE_MISMATCH");
  });

  it("throws RESPONSE_MISMATCH when tracking_number in response does not match request", async () => {
    // Response tracking is "SHIPPO_DELIVERED", but we requested "WRONG_TRACKING"
    const att = buildSignedAttestation({ carrier: "shippo", tracking: "SHIPPO_DELIVERED" });
    const client = new OracleClient("https://oracle.example.com", {
      fetchFn: makeFetchFn(att),
    });

    let error: unknown;
    try {
      await client.prepareCommitment({}, "shippo", "WRONG_TRACKING");
    } catch (e) {
      error = e;
    }

    expect(error).toBeInstanceOf(OracleSdkError);
    expect((error as OracleSdkError).code).toBe("RESPONSE_MISMATCH");
  });

  it("throws OracleSdkError(API) on non-2xx response", async () => {
    const client = new OracleClient("https://oracle.example.com", {
      fetchFn: makeTextFetchFn("upstream boom", 502),
    });

    let error: unknown;
    try {
      await client.prepareCommitment({}, "shippo", "SHIPPO_DELIVERED");
    } catch (e) {
      error = e;
    }

    expect(error).toBeInstanceOf(OracleSdkError);
    expect((error as OracleSdkError).code).toBe("API");
    expect((error as OracleSdkError).message).toContain("upstream boom");
  });

  it("runs verifyAttestation and throws if signature is invalid", async () => {
    const att = buildSignedAttestation();
    // Corrupt the signature so verifyAttestation rejects it
    const corrupted: OracleAttestation = {
      ...att,
      signature: "00".repeat(64),
    };
    const client = new OracleClient("https://oracle.example.com", {
      fetchFn: makeFetchFn(corrupted),
    });

    let error: unknown;
    try {
      await client.prepareCommitment({}, "shippo", "SHIPPO_DELIVERED");
    } catch (e) {
      error = e;
    }

    expect(error).toBeInstanceOf(OracleSdkError);
    expect((error as OracleSdkError).code).toBe("INVALID_SIGNATURE");
  });
});
