use blake2::{Blake2b, Digest, digest::consts::U32};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use pallas::codec::minicbor;
use pallas::ledger::primitives::{BigInt, BoundedBytes, Constr, Int, MaybeIndefArray, PlutusData};

use crate::error::OracleSdkError;
use crate::models::{OracleAttestation, OracleStatus};

pub(crate) fn decode_hex(field: &'static str, value: &str) -> Result<Vec<u8>, OracleSdkError> {
    hex::decode(value).map_err(|source| OracleSdkError::InvalidHex { field, source })
}

pub(crate) fn decode_array<const N: usize>(
    field: &'static str,
    value: &str,
) -> Result<[u8; N], OracleSdkError> {
    let bytes = decode_hex(field, value)?;
    let actual = bytes.len();
    bytes.try_into().map_err(|_| OracleSdkError::InvalidLength {
        field,
        expected: N,
        actual,
    })
}

pub(crate) fn blake2b256(input: &[u8]) -> Vec<u8> {
    let mut hasher = Blake2b::<U32>::new();
    hasher.update(input);
    hasher.finalize().to_vec()
}

fn plutus_oracle_data(
    carrier_hash: Vec<u8>,
    tracking_number_hash: Vec<u8>,
    status: OracleStatus,
    timestamp: i64,
) -> PlutusData {
    PlutusData::Constr(Constr {
        tag: 121,
        any_constructor: None,
        fields: MaybeIndefArray::Indef(vec![
            PlutusData::BoundedBytes(BoundedBytes::from(carrier_hash)),
            PlutusData::BoundedBytes(BoundedBytes::from(tracking_number_hash)),
            PlutusData::BoundedBytes(BoundedBytes::from(status.as_bytes().to_vec())),
            PlutusData::BigInt(BigInt::Int(Int::from(timestamp))),
        ]),
    })
}

pub(crate) fn verify_attestation(
    attestation: &OracleAttestation,
    expected_public_key: Option<&[u8; 32]>,
) -> Result<(), OracleSdkError> {
    let public_key_bytes = decode_array::<32>("public_key", &attestation.public_key)?;
    if let Some(expected) = expected_public_key {
        if public_key_bytes != *expected {
            return Err(OracleSdkError::UnexpectedPublicKey);
        }
    }

    let verifying_key =
        VerifyingKey::from_bytes(&public_key_bytes).map_err(|_| OracleSdkError::InvalidLength {
            field: "public_key",
            expected: 32,
            actual: public_key_bytes.len(),
        })?;
    let signature_bytes = decode_array::<64>("signature", &attestation.signature)?;
    let signature = Signature::from_bytes(&signature_bytes);
    let cbor_bytes = decode_hex("cbor_hex", &attestation.cbor_hex)?;

    verifying_key
        .verify(&cbor_bytes, &signature)
        .map_err(|_| OracleSdkError::InvalidSignature)?;

    let carrier_hash_bytes =
        decode_array::<32>("data.carrier_hash", &attestation.data.carrier_hash)?;
    let tracking_hash_bytes = decode_array::<32>(
        "data.tracking_number_hash",
        &attestation.data.tracking_number_hash,
    )?;
    let expected_cbor = minicbor::to_vec(&plutus_oracle_data(
        carrier_hash_bytes.to_vec(),
        tracking_hash_bytes.to_vec(),
        attestation.data.status,
        attestation.data.timestamp,
    ))
    .map_err(|_| OracleSdkError::CborMismatch)?;

    if expected_cbor != cbor_bytes {
        return Err(OracleSdkError::CborMismatch);
    }

    let carrier_hash = blake2b256(attestation.plaintext.carrier.as_bytes());
    if carrier_hash != carrier_hash_bytes {
        return Err(OracleSdkError::CarrierHashMismatch);
    }

    let tracking_hash = blake2b256(attestation.plaintext.tracking_number.as_bytes());
    if tracking_hash != tracking_hash_bytes {
        return Err(OracleSdkError::TrackingNumberHashMismatch);
    }

    Ok(())
}
