use ed25519_dalek::{Signer, SigningKey};
use pallas::codec::minicbor;
use pallas::ledger::primitives::{BigInt, BoundedBytes, Constr, Int, MaybeIndefArray, PlutusData};

// Produces deterministic Ed25519 signing material for the "valid signature"
// test vector in `onchain/validators/oracle.ak`. Keep the hex constants here
// in sync with the ones hardcoded in the Aiken test — if any of these change,
// the on-chain test must be updated too.

fn oracle_data_zero_edge() -> PlutusData {
    PlutusData::Constr(Constr {
        tag: 121,
        any_constructor: None,
        fields: MaybeIndefArray::Indef(vec![
            PlutusData::BoundedBytes(BoundedBytes::from(Vec::new())),
            PlutusData::BoundedBytes(BoundedBytes::from(Vec::new())),
            PlutusData::BoundedBytes(BoundedBytes::from(Vec::new())),
            PlutusData::BigInt(BigInt::Int(Int::from(0_i64))),
        ]),
    })
}

#[test]
fn zero_edge_signature_vector() {
    let sk = SigningKey::from_bytes(&[1u8; 32]);
    let vk = sk.verifying_key();

    let data = oracle_data_zero_edge();
    let cbor = minicbor::to_vec(&data).expect("plutus data must encode");
    let sig = sk.sign(&cbor);

    assert_eq!(
        hex::encode(cbor),
        "d8799f40404000ff",
        "cbor must match the Aiken-derived canonical encoding"
    );
    assert_eq!(
        hex::encode(vk.to_bytes()),
        "8a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c"
    );
    assert_eq!(
        hex::encode(sig.to_bytes()),
        "4b2d1bcbbb6ab6d3f73271c50642306702899b7c979d48f86106092860e4d978e0e12924b7520049fabfce8838342faf6a5361e20b45be7ee9da31706967f104"
    );
}
