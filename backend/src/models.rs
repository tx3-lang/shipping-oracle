use serde::{Deserialize, Serialize};

// ---- Shippo API (external) -------------------------------------------------

/// Shippo API tracking response (partial, only fields we need).
#[derive(Debug, Deserialize)]
pub struct TrackingResponse {
    pub carrier: String,
    pub tracking_number: String,
    pub tracking_status: TrackingStatus,
}

/// Shippo API tracking status (partial).
#[derive(Debug, Deserialize)]
pub struct TrackingStatus {
    pub status: String,
    #[serde(default)]
    pub status_details: String,
}

// ---- Oracle HTTP API -------------------------------------------------------

/// Query parameters for `GET /v1/shipment`.
#[derive(Debug, Deserialize)]
pub struct ShipmentQuery {
    pub carrier: String,
    pub tracking_number: String,
}

/// Hashed identifiers that go on-chain (no PII — milestone B1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OracleData {
    /// blake2b-256 hash of carrier (hex).
    pub carrier_hash: String,
    /// blake2b-256 hash of tracking_number (hex).
    pub tracking_number_hash: String,
    pub status: String,
    pub timestamp: i64,
}

/// Plaintext identifiers returned to the consumer for UX; never signed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShipmentPlaintext {
    pub carrier: String,
    pub tracking_number: String,
}

/// Response body for `GET /v1/shipment`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedOracleResponse {
    pub data: OracleData,
    pub plaintext: ShipmentPlaintext,
    /// Ed25519 signature over `cbor_hex` bytes.
    pub signature: String,
    /// Oracle verifying key.
    pub public_key: String,
    /// Canonical CBOR of the PlutusData form of `data`. Consumers should
    /// embed these bytes verbatim in the on-chain redeemer — re-serializing
    /// them would risk a mismatch with the signed message.
    pub cbor_hex: String,
}
