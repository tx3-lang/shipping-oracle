use thiserror::Error;

#[derive(Debug, Error)]
pub enum OracleSdkError {
    #[error("http request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("oracle returned http {status}: {message}")]
    Api { status: u16, message: String },

    #[error("invalid hex in {field}: {source}")]
    InvalidHex {
        field: &'static str,
        #[source]
        source: hex::FromHexError,
    },

    #[error("{field} must decode to {expected} bytes, got {actual}")]
    InvalidLength {
        field: &'static str,
        expected: usize,
        actual: usize,
    },

    #[error("response {field} mismatch: expected {expected}, got {actual}")]
    ResponseMismatch {
        field: &'static str,
        expected: String,
        actual: String,
    },

    #[error("public key does not match the expected oracle key")]
    UnexpectedPublicKey,

    #[error("attestation signature failed verification")]
    InvalidSignature,

    #[error("attestation cbor does not match the declared OracleData fields")]
    CborMismatch,

    #[error("plaintext carrier does not hash to data.carrier_hash")]
    CarrierHashMismatch,

    #[error("plaintext tracking_number does not hash to data.tracking_number_hash")]
    TrackingNumberHashMismatch,
}
