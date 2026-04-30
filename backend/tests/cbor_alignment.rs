use pallas::codec::minicbor;
use pallas::ledger::primitives::{BigInt, BoundedBytes, Constr, Int, MaybeIndefArray, PlutusData};

// Expected hex values are produced by Aiken `builtin.serialise_data` (via
// `cbor.serialise`) in `onchain/lib/cbor_alignment_tests.ak`. Keep the two
// files in sync: if Aiken's output changes, update both sides together.

fn oracle_data_as_plutus(
    carrier_hash: Vec<u8>,
    tracking_number_hash: Vec<u8>,
    status: &[u8],
    timestamp: i64,
) -> PlutusData {
    PlutusData::Constr(Constr {
        tag: 121,
        any_constructor: None,
        fields: MaybeIndefArray::Indef(vec![
            PlutusData::BoundedBytes(BoundedBytes::from(carrier_hash)),
            PlutusData::BoundedBytes(BoundedBytes::from(tracking_number_hash)),
            PlutusData::BoundedBytes(BoundedBytes::from(status.to_vec())),
            PlutusData::BigInt(BigInt::Int(Int::from(timestamp))),
        ]),
    })
}

fn cbor_hex(data: &PlutusData) -> String {
    hex::encode(minicbor::to_vec(data).expect("plutus data must encode"))
}

#[test]
fn oracle_data_delivered_cbor() {
    let data = oracle_data_as_plutus(vec![0x11; 32], vec![0x22; 32], b"DELIVERED", 1712000000);
    assert_eq!(
        cbor_hex(&data),
        "d8799f58201111111111111111111111111111111111111111111111111111111111111111582022222222222222222222222222222222222222222222222222222222222222224944454c4956455245441a660b0c00ff"
    );
}

#[test]
fn oracle_data_unknown_cbor() {
    let data = oracle_data_as_plutus(vec![0x33; 32], vec![0x44; 32], b"UNKNOWN", 1712345678);
    assert_eq!(
        cbor_hex(&data),
        "d8799f582033333333333333333333333333333333333333333333333333333333333333335820444444444444444444444444444444444444444444444444444444444444444447554e4b4e4f574e1a6610524eff"
    );
}

#[test]
fn oracle_data_zero_edge_cbor() {
    let data = oracle_data_as_plutus(vec![], vec![], b"", 0);
    assert_eq!(cbor_hex(&data), "d8799f40404000ff");
}
