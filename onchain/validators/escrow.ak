use aiken/collection/list
use aiken/interval
use cardano/address.{Address, VerificationKey}
use cardano/assets
use cardano/transaction.{
  InlineDatum, Input, NoDatum, Output, OutputReference, Transaction, find_input,
}
use config
use oracle_verification
use types.{
  EscrowDatum, EscrowRedeemer, GovernanceDatum, OracleData, Refund, Release,
  status_delivered,
}

validator escrow {
  spend(
    datum: Option<EscrowDatum>,
    redeemer: EscrowRedeemer,
    own_ref: OutputReference,
    tx: Transaction,
  ) {
    expect Some(escrow_datum) = datum
    expect Some(own_input) = tx.inputs |> find_input(own_ref)

    when redeemer is {
      Release { data, signature } -> and {
          data.status == status_delivered,
          data.carrier_hash == escrow_datum.carrier_hash,
          data.tracking_number_hash == escrow_datum.tracking_number_hash,
          oracle_verification.verify(tx, data, signature),
          paid_to(
            tx,
            escrow_datum.merchant,
            assets.lovelace_of(own_input.output.value),
          ),
        }
      Refund -> and {
          interval.after(escrow_datum.refund_after)
            |> interval.includes(tx.validity_range),
          paid_to(
            tx,
            escrow_datum.buyer,
            assets.lovelace_of(own_input.output.value),
          ),
        }
    }
  }

  else(_) {
    fail @"unsupported purpose"
  }
}

fn paid_to(tx: Transaction, pkh: ByteArray, minimum_lovelace: Int) -> Bool {
  list.any(
    tx.outputs,
    fn(output) {
      output.address.payment_credential == VerificationKey(pkh) && assets.lovelace_of(
        output.value,
      ) >= minimum_lovelace
    },
  )
}

// ---- tests ----

const test_oracle_vk: ByteArray =
  #"8a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c"

const test_buyer_pkh: ByteArray =
  #"11111111111111111111111111111111111111111111111111111111"

const test_merchant_pkh: ByteArray =
  #"22222222222222222222222222222222222222222222222222222222"

const test_carrier_hash: ByteArray =
  #"3333333333333333333333333333333333333333333333333333333333333333"

const test_tracking_hash: ByteArray =
  #"4444444444444444444444444444444444444444444444444444444444444444"

const test_delivered_signature: ByteArray =
  #"6380c10997a2b81dff56959bacef76b9590b86cc798a915adbb0dfe1488ac899ea77dccfd234a9e70e13182a0962586d6d725af9d53f4031eac32a26923be20b"

fn test_escrow_datum() -> EscrowDatum {
  EscrowDatum {
    buyer: test_buyer_pkh,
    merchant: test_merchant_pkh,
    order_id: "order-123",
    carrier_hash: test_carrier_hash,
    tracking_number_hash: test_tracking_hash,
    paid_at: 1_712_000_000,
    refund_after: 1_713_000_000,
  }
}

fn test_oracle_data(status: ByteArray) -> OracleData {
  OracleData {
    carrier_hash: test_carrier_hash,
    tracking_number_hash: test_tracking_hash,
    status,
    timestamp: 1_712_000_000,
  }
}

fn address_from_pkh(pkh: ByteArray) -> Address {
  Address { payment_credential: VerificationKey(pkh), stake_credential: None }
}

fn own_ref() -> OutputReference {
  OutputReference {
    transaction_id: #"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    output_index: 0,
  }
}

fn escrow_input() -> Input {
  Input {
    output_reference: own_ref(),
    output: Output {
      address: address_from_pkh(
        #"eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
      ),
      value: assets.from_lovelace(10_000_000),
      datum: NoDatum,
      reference_script: None,
    },
  }
}

fn gov_input() -> Input {
  Input {
    output_reference: OutputReference {
      transaction_id: #"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
      output_index: 0,
    },
    output: Output {
      address: address_from_pkh(
        #"99999999999999999999999999999999999999999999999999999999",
      ),
      value: assets.from_asset(
        config.gov_policy_id,
        config.governance_asset_name,
        1,
      ),
      datum: InlineDatum(GovernanceDatum { oracle_vk: test_oracle_vk }),
      reference_script: None,
    },
  }
}

fn payout_output(pkh: ByteArray, amount: Int) -> Output {
  Output {
    address: address_from_pkh(pkh),
    value: assets.from_lovelace(amount),
    datum: NoDatum,
    reference_script: None,
  }
}

test release_valid_delivered_signature() {
  let redeemer =
    Release {
      data: test_oracle_data(status_delivered),
      signature: test_delivered_signature,
    }
  let tx =
    Transaction {
      ..transaction.placeholder,
      inputs: [escrow_input()],
      reference_inputs: [gov_input()],
      outputs: [payout_output(test_merchant_pkh, 10_000_000)],
    }
  escrow.spend(Some(test_escrow_datum()), redeemer, own_ref(), tx)
}

test release_rejects_non_delivered_status() fail {
  let redeemer =
    Release {
      data: test_oracle_data("IN_TRANSIT"),
      signature: test_delivered_signature,
    }
  let tx =
    Transaction {
      ..transaction.placeholder,
      inputs: [escrow_input()],
      reference_inputs: [gov_input()],
      outputs: [payout_output(test_merchant_pkh, 10_000_000)],
    }
  escrow.spend(Some(test_escrow_datum()), redeemer, own_ref(), tx)
}

test release_rejects_wrong_payout() fail {
  let redeemer =
    Release {
      data: test_oracle_data(status_delivered),
      signature: test_delivered_signature,
    }
  let tx =
    Transaction {
      ..transaction.placeholder,
      inputs: [escrow_input()],
      reference_inputs: [gov_input()],
      outputs: [payout_output(test_buyer_pkh, 10_000_000)],
    }
  escrow.spend(Some(test_escrow_datum()), redeemer, own_ref(), tx)
}

test refund_valid_after_timeout() {
  let tx =
    Transaction {
      ..transaction.placeholder,
      inputs: [escrow_input()],
      outputs: [payout_output(test_buyer_pkh, 10_000_000)],
      validity_range: interval.after(1_713_000_000),
    }
  escrow.spend(Some(test_escrow_datum()), Refund, own_ref(), tx)
}

test refund_rejects_before_timeout() fail {
  let tx =
    Transaction {
      ..transaction.placeholder,
      inputs: [escrow_input()],
      outputs: [payout_output(test_buyer_pkh, 10_000_000)],
      validity_range: interval.before(1_713_000_000),
    }
  escrow.spend(Some(test_escrow_datum()), Refund, own_ref(), tx)
}
