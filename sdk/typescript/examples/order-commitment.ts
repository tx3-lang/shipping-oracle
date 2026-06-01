/**
 * Example: link an application order to an on-chain shipment tracking commitment.
 *
 * Mirrors sdk/rust/examples/order_commitment.rs.
 *
 * Usage:
 *   ORACLE_BASE_URL=http://127.0.0.1:3000 \
 *   ORACLE_PUBLIC_KEY=<hex> \
 *   node --loader ts-node/esm examples/order-commitment.ts
 *
 * Or with tsx:
 *   pnpm exec tsx examples/order-commitment.ts
 */

// In-repo example imports from source. In your own project, import from the
// package instead: `import { OracleClient } from "shipping-oracle-sdk";`
import { OracleClient } from "../src/index.js";

interface OrderContext {
  orderId: string;
}

async function main(): Promise<void> {
  const baseUrl =
    process.env["ORACLE_BASE_URL"] ?? "http://127.0.0.1:3000";
  const expectedPublicKeyHex = process.env["ORACLE_PUBLIC_KEY"];

  const client = new OracleClient(baseUrl, { expectedPublicKeyHex });

  const commitment = await client.prepareCommitment<OrderContext>(
    { orderId: "ord_123" },
    "shippo",
    "SHIPPO_DELIVERED"
  );

  const { context, attestation } = commitment;

  console.log("linked order   :", context.orderId);
  console.log("status         :", attestation.data.status);
  console.log("carrier_hash   :", attestation.data.carrier_hash);
  console.log("timestamp      :", attestation.data.timestamp);
  console.log("public_key     :", attestation.public_key);
}

main().catch((err: unknown) => {
  console.error("error:", err);
  process.exit(1);
});
