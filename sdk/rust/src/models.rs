use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::OracleSdkError;
use crate::tx3::{ConsumeOracleDataArgsJson, ConsumeOracleDataParams};
use crate::verify::{decode_array, verify_attestation};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShipmentReference {
    pub carrier: String,
    pub tracking_number: String,
}

impl ShipmentReference {
    pub fn new(carrier: impl Into<String>, tracking_number: impl Into<String>) -> Self {
        Self {
            carrier: carrier.into(),
            tracking_number: tracking_number.into(),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OracleStatus {
    Delivered,
    NotDelivered,
    InTransit,
    PreTransit,
    Unknown,
}

impl OracleStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Delivered => "DELIVERED",
            Self::NotDelivered => "NOT_DELIVERED",
            Self::InTransit => "IN_TRANSIT",
            Self::PreTransit => "PRE_TRANSIT",
            Self::Unknown => "UNKNOWN",
        }
    }

    pub fn as_bytes(&self) -> &'static [u8] {
        self.as_str().as_bytes()
    }
}

impl fmt::Display for OracleStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OracleData {
    pub carrier_hash: String,
    pub tracking_number_hash: String,
    pub status: OracleStatus,
    pub timestamp: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShipmentPlaintext {
    pub carrier: String,
    pub tracking_number: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OracleAttestation {
    pub data: OracleData,
    pub plaintext: ShipmentPlaintext,
    pub signature: String,
    pub public_key: String,
    pub cbor_hex: String,
}

impl OracleAttestation {
    pub fn shipment_reference(&self) -> ShipmentReference {
        ShipmentReference::new(
            self.plaintext.carrier.clone(),
            self.plaintext.tracking_number.clone(),
        )
    }

    pub fn verify(&self) -> Result<(), OracleSdkError> {
        verify_attestation(self, None)
    }

    pub fn verify_with_expected_public_key_hex(
        &self,
        expected_public_key_hex: &str,
    ) -> Result<(), OracleSdkError> {
        let expected_public_key =
            decode_array::<32>("expected_public_key", expected_public_key_hex)?;
        verify_attestation(self, Some(&expected_public_key))
    }

    pub fn to_consume_oracle_data_params(&self) -> Result<ConsumeOracleDataParams, OracleSdkError> {
        ConsumeOracleDataParams::try_from(self)
    }

    pub fn to_cli_args_json(&self) -> Result<ConsumeOracleDataArgsJson, OracleSdkError> {
        Ok(self.to_consume_oracle_data_params()?.to_cli_args_json())
    }
}

#[derive(Debug)]
pub struct PreparedCommitment<TContext> {
    pub context: TContext,
    pub attestation: OracleAttestation,
    expected_public_key: Option<[u8; 32]>,
}

impl<TContext> PreparedCommitment<TContext> {
    pub(crate) fn new(
        context: TContext,
        attestation: OracleAttestation,
        expected_public_key: Option<[u8; 32]>,
    ) -> Self {
        Self {
            context,
            attestation,
            expected_public_key,
        }
    }

    pub fn verify(&self) -> Result<(), OracleSdkError> {
        verify_attestation(&self.attestation, self.expected_public_key.as_ref())
    }

    pub fn verify_with_expected_public_key_hex(
        &self,
        expected_public_key_hex: &str,
    ) -> Result<(), OracleSdkError> {
        self.attestation
            .verify_with_expected_public_key_hex(expected_public_key_hex)
    }

    pub fn to_consume_oracle_data_params(&self) -> Result<ConsumeOracleDataParams, OracleSdkError> {
        self.attestation.to_consume_oracle_data_params()
    }

    pub fn to_cli_args_json(&self) -> Result<ConsumeOracleDataArgsJson, OracleSdkError> {
        self.attestation.to_cli_args_json()
    }

    pub fn into_parts(self) -> (TContext, OracleAttestation) {
        (self.context, self.attestation)
    }
}
