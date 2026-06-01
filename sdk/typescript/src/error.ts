/**
 * Error type for the Shipping Oracle SDK.
 * Mirrors the variant names from sdk/rust/src/error.rs.
 */

/** Discriminated error codes covering every failure mode in the SDK. */
export type OracleSdkErrorCode =
  | "API"
  | "INVALID_LENGTH"
  | "RESPONSE_MISMATCH"
  | "UNEXPECTED_PUBLIC_KEY"
  | "INVALID_SIGNATURE"
  | "CBOR_MISMATCH"
  | "CARRIER_HASH_MISMATCH"
  | "TRACKING_NUMBER_HASH_MISMATCH";

/** Structured error thrown by the Shipping Oracle SDK. */
export class OracleSdkError extends Error {
  override readonly name = "OracleSdkError";
  readonly code: OracleSdkErrorCode;

  constructor(code: OracleSdkErrorCode, message: string) {
    super(message);
    this.code = code;
    // Restore prototype chain in transpiled environments (e.g. ts-node, Babel).
    Object.setPrototypeOf(this, new.target.prototype);
  }
}
