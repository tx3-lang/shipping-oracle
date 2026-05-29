use anyhow::{Context, Result, bail};
use blake2::{Blake2b, Digest, digest::consts::U32};
use ed25519_dalek::{Signer, SigningKey};
use pallas::codec::minicbor;
use pallas::ledger::primitives::{BigInt, BoundedBytes, Constr, Int, MaybeIndefArray, PlutusData};

use crate::models::{OracleData, ShipmentPlaintext, SignedOracleResponse};
use crate::shipment::{self, ShipmentClient};

pub struct OracleService {
    shipment_client: ShipmentClient,
    signing_key: SigningKey,
    clock: Clock,
}

pub type Clock = Box<dyn Fn() -> i64 + Send + Sync>;

impl OracleService {
    pub fn new(shipment_client: ShipmentClient, signing_key: SigningKey) -> Self {
        Self {
            shipment_client,
            signing_key,
            clock: Box::new(|| chrono::Utc::now().timestamp()),
        }
    }

    pub fn with_clock(
        shipment_client: ShipmentClient,
        signing_key: SigningKey,
        clock: Clock,
    ) -> Self {
        Self {
            shipment_client,
            signing_key,
            clock,
        }
    }

    pub async fn attest(
        &self,
        carrier: &str,
        tracking_number: &str,
    ) -> Result<SignedOracleResponse> {
        let tracking_status = self
            .shipment_client
            .fetch_shipment_status(carrier, tracking_number)
            .await?;
        let status = shipment::normalize_status(&tracking_status);
        let timestamp = (self.clock)();

        let carrier_hash_bytes = blake2b256(carrier.as_bytes());
        let tracking_hash_bytes = blake2b256(tracking_number.as_bytes());
        let status_bytes = status.as_bytes().to_vec();

        let plutus = plutus_oracle_data(
            carrier_hash_bytes.clone(),
            tracking_hash_bytes.clone(),
            status_bytes,
            timestamp,
        );
        let cbor_bytes = minicbor::to_vec(&plutus).context("encoding OracleData as CBOR")?;
        let signature = self.signing_key.sign(&cbor_bytes);
        let public_key = self.signing_key.verifying_key();

        Ok(SignedOracleResponse {
            data: OracleData {
                carrier_hash: hex::encode(&carrier_hash_bytes),
                tracking_number_hash: hex::encode(&tracking_hash_bytes),
                status,
                timestamp,
            },
            plaintext: ShipmentPlaintext {
                carrier: carrier.to_string(),
                tracking_number: tracking_number.to_string(),
            },
            signature: hex::encode(signature.to_bytes()),
            public_key: hex::encode(public_key.to_bytes()),
            cbor_hex: hex::encode(cbor_bytes),
        })
    }
}

pub fn load_signing_key(hex_sk: &str) -> Result<SigningKey> {
    let bytes = hex::decode(hex_sk).context("ORACLE_SK must be hex")?;
    if bytes.len() != 32 {
        bail!("ORACLE_SK must decode to 32 bytes, got {}", bytes.len());
    }
    let mut fixed = [0u8; 32];
    fixed.copy_from_slice(&bytes);
    Ok(SigningKey::from_bytes(&fixed))
}

pub fn blake2b256(input: &[u8]) -> Vec<u8> {
    let mut hasher = Blake2b::<U32>::new();
    hasher.update(input);
    hasher.finalize().to_vec()
}

/// Build the PlutusData encoding used on-chain. Matches `cbor_alignment` tests:
/// Constr(0, [carrier_hash, tracking_number_hash, status, timestamp]) with an
/// indefinite-length fields array — byte-identical to Aiken's `serialise_data`.
pub fn plutus_oracle_data(
    carrier_hash: Vec<u8>,
    tracking_number_hash: Vec<u8>,
    status: Vec<u8>,
    timestamp: i64,
) -> PlutusData {
    PlutusData::Constr(Constr {
        tag: 121,
        any_constructor: None,
        fields: MaybeIndefArray::Indef(vec![
            PlutusData::BoundedBytes(BoundedBytes::from(carrier_hash)),
            PlutusData::BoundedBytes(BoundedBytes::from(tracking_number_hash)),
            PlutusData::BoundedBytes(BoundedBytes::from(status)),
            PlutusData::BigInt(BigInt::Int(Int::from(timestamp))),
        ]),
    })
}
