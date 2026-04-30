use aiken/cbor
use types.{OracleData}

// Test vectors that lock in canonical CBOR produced by `builtin.serialise_data`
// (via `cbor.serialise`) for `OracleData`. Rust side (pallas minicbor) must
// produce the same bytes for the same values. If either side drifts, Ed25519
// signatures will silently fail on-chain.
//
// Shape observed: Constr(0, [...]) => tag 121 (d879) + indefinite-length array
// (9f ... ff). Integers use standard CBOR majors. ByteArray uses major 2.

test oracle_data_delivered_cbor() {
  let data =
    OracleData {
      carrier_hash: #"1111111111111111111111111111111111111111111111111111111111111111",
      tracking_number_hash: #"2222222222222222222222222222222222222222222222222222222222222222",
      status: "DELIVERED",
      timestamp: 1712000000,
    }
  cbor.serialise(data) == #"d8799f58201111111111111111111111111111111111111111111111111111111111111111582022222222222222222222222222222222222222222222222222222222222222224944454c4956455245441a660b0c00ff"
}

test oracle_data_unknown_cbor() {
  let data =
    OracleData {
      carrier_hash: #"3333333333333333333333333333333333333333333333333333333333333333",
      tracking_number_hash: #"4444444444444444444444444444444444444444444444444444444444444444",
      status: "UNKNOWN",
      timestamp: 1712345678,
    }
  cbor.serialise(data) == #"d8799f582033333333333333333333333333333333333333333333333333333333333333335820444444444444444444444444444444444444444444444444444444444444444447554e4b4e4f574e1a6610524eff"
}

test oracle_data_zero_edge_cbor() {
  let data =
    OracleData {
      carrier_hash: #"",
      tracking_number_hash: #"",
      status: "",
      timestamp: 0,
    }
  cbor.serialise(data) == #"d8799f40404000ff"
}
