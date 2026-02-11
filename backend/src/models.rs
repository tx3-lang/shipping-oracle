use pallas::ledger::addresses::Address;
use serde::Deserialize;

/// Shippo API tracking response (partial, only fields we need)
#[derive(Debug, Deserialize)]
pub struct TrackingResponse {
    pub carrier: String,
    pub tracking_number: String,
    pub tracking_status: TrackingStatus,
}

/// Shippo API tracking status (partial, only fields we need)
#[derive(Debug, Deserialize)]
pub struct TrackingStatus {
    pub status: String,           // e.g., "DELIVERED", "TRANSIT", "PRE_TRANSIT"
    pub status_details: String,   // Descriptive message
}

/// Represents a tracking UTxO
#[derive(Debug, Clone)]
pub struct TrackingUTxO {
    pub tx_hash: String,
    pub tx_index: u32,
    pub datum: TrackingDatum,
}

/// On-chain tracking datum structure
#[derive(Debug, Clone)]
pub struct TrackingDatum {
    pub carrier: String,
    pub tracking_number: String,
    pub outbox_address: Address,
}
