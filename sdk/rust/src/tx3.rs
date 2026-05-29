use serde::Serialize;
use tx3_sdk::core::ArgMap;

use crate::error::OracleSdkError;
use crate::models::OracleAttestation;
use crate::verify::{decode_array, decode_hex};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConsumeOracleDataParams {
    pub p_carrier_hash: Vec<u8>,
    pub p_tracking_number_hash: Vec<u8>,
    pub p_status: Vec<u8>,
    pub p_timestamp: i64,
    pub p_signature: Vec<u8>,
}

impl ConsumeOracleDataParams {
    pub fn to_cli_args_json(&self) -> ConsumeOracleDataArgsJson {
        ConsumeOracleDataArgsJson {
            p_carrier_hash: hex::encode(&self.p_carrier_hash),
            p_tracking_number_hash: hex::encode(&self.p_tracking_number_hash),
            p_status: hex::encode(&self.p_status),
            p_timestamp: self.p_timestamp,
            p_signature: hex::encode(&self.p_signature),
        }
    }
}

impl TryFrom<&OracleAttestation> for ConsumeOracleDataParams {
    type Error = OracleSdkError;

    fn try_from(attestation: &OracleAttestation) -> Result<Self, Self::Error> {
        Ok(Self {
            p_carrier_hash: decode_array::<32>(
                "data.carrier_hash",
                &attestation.data.carrier_hash,
            )?
            .to_vec(),
            p_tracking_number_hash: decode_array::<32>(
                "data.tracking_number_hash",
                &attestation.data.tracking_number_hash,
            )?
            .to_vec(),
            p_status: attestation.data.status.as_bytes().to_vec(),
            p_timestamp: attestation.data.timestamp,
            p_signature: decode_array::<64>("signature", &attestation.signature)?.to_vec(),
        })
    }
}

impl TryFrom<OracleAttestation> for ConsumeOracleDataParams {
    type Error = OracleSdkError;

    fn try_from(attestation: OracleAttestation) -> Result<Self, Self::Error> {
        Self::try_from(&attestation)
    }
}

impl From<ConsumeOracleDataParams> for ArgMap {
    fn from(args: ConsumeOracleDataParams) -> Self {
        let mut map = ArgMap::new();

        map.insert(
            "p_carrier_hash".to_string(),
            serde_json::to_value(&args.p_carrier_hash).expect("failed to serialize tx arg"),
        );
        map.insert(
            "p_tracking_number_hash".to_string(),
            serde_json::to_value(&args.p_tracking_number_hash).expect("failed to serialize tx arg"),
        );
        map.insert(
            "p_status".to_string(),
            serde_json::to_value(&args.p_status).expect("failed to serialize tx arg"),
        );
        map.insert(
            "p_timestamp".to_string(),
            serde_json::to_value(&args.p_timestamp).expect("failed to serialize tx arg"),
        );
        map.insert(
            "p_signature".to_string(),
            serde_json::to_value(&args.p_signature).expect("failed to serialize tx arg"),
        );

        map
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConsumeOracleDataArgsJson {
    pub p_carrier_hash: String,
    pub p_tracking_number_hash: String,
    pub p_status: String,
    pub p_timestamp: i64,
    pub p_signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReleaseEscrowArgsJson {
    pub escrow_utxo: String,
    pub p_carrier_hash: String,
    pub p_tracking_number_hash: String,
    pub p_status: String,
    pub p_timestamp: i64,
    pub p_signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LockEscrowAdaArgsJson {
    pub quantity: i64,
    pub buyer_pkh: String,
    pub merchant_pkh: String,
    pub order_id: String,
    pub carrier_hash: String,
    pub tracking_number_hash: String,
    pub paid_at: i64,
    pub refund_after: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RefundEscrowArgsJson {
    pub escrow_utxo: String,
}

impl ReleaseEscrowArgsJson {
    pub fn as_json_string(&self) -> String {
        serde_json::to_string_pretty(self).expect("json serialization must succeed")
    }

    pub fn write_to_path(&self, path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
        std::fs::write(path, self.as_json_string())
    }
}

impl LockEscrowAdaArgsJson {
    pub fn as_json_string(&self) -> String {
        serde_json::to_string_pretty(self).expect("json serialization must succeed")
    }

    pub fn write_to_path(&self, path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
        std::fs::write(path, self.as_json_string())
    }
}

impl RefundEscrowArgsJson {
    pub fn as_json_string(&self) -> String {
        serde_json::to_string_pretty(self).expect("json serialization must succeed")
    }

    pub fn write_to_path(&self, path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
        std::fs::write(path, self.as_json_string())
    }
}

impl ConsumeOracleDataArgsJson {
    pub fn as_json_string(&self) -> String {
        serde_json::to_string_pretty(self).expect("json serialization must succeed")
    }

    pub fn write_to_path(&self, path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
        std::fs::write(path, self.as_json_string())
    }
}

pub fn cbor_bytes(attestation: &OracleAttestation) -> Result<Vec<u8>, OracleSdkError> {
    decode_hex("cbor_hex", &attestation.cbor_hex)
}

pub fn release_escrow_args_json(
    escrow_utxo: impl Into<String>,
    attestation: &OracleAttestation,
) -> ReleaseEscrowArgsJson {
    ReleaseEscrowArgsJson {
        escrow_utxo: escrow_utxo.into(),
        p_carrier_hash: attestation.data.carrier_hash.clone(),
        p_tracking_number_hash: attestation.data.tracking_number_hash.clone(),
        p_status: hex::encode(attestation.data.status.as_bytes()),
        p_timestamp: attestation.data.timestamp,
        p_signature: attestation.signature.clone(),
    }
}

pub fn lock_escrow_ada_args_json(
    quantity: i64,
    buyer_pkh: impl Into<String>,
    merchant_pkh: impl Into<String>,
    order_id: impl AsRef<str>,
    paid_at: i64,
    refund_after: i64,
    attestation: &OracleAttestation,
) -> LockEscrowAdaArgsJson {
    LockEscrowAdaArgsJson {
        quantity,
        buyer_pkh: buyer_pkh.into(),
        merchant_pkh: merchant_pkh.into(),
        order_id: hex::encode(order_id.as_ref().as_bytes()),
        carrier_hash: attestation.data.carrier_hash.clone(),
        tracking_number_hash: attestation.data.tracking_number_hash.clone(),
        paid_at,
        refund_after,
    }
}

pub fn refund_escrow_args_json(escrow_utxo: impl Into<String>) -> RefundEscrowArgsJson {
    RefundEscrowArgsJson {
        escrow_utxo: escrow_utxo.into(),
    }
}
