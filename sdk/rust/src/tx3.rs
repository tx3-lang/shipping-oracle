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

impl ConsumeOracleDataArgsJson {
    pub fn as_json_string(&self) -> String {
        serde_json::to_string_pretty(self).expect("json serialization must succeed")
    }
}

pub fn cbor_bytes(attestation: &OracleAttestation) -> Result<Vec<u8>, OracleSdkError> {
    decode_hex("cbor_hex", &attestation.cbor_hex)
}
