use aiken/collection/list
use cardano/address
use cardano/assets
use cardano/script_context.{ScriptContext}
use cardano/transaction.{InlineDatum, OutputReference, Transaction}
use config
use types.{
  ConsumeTracking, ShipmentDatum, TrackingDatum, TrackingRedeemer,
  status_delivered, status_not_delivered,
}

validator tracking {
  spend(
    datum: Option<TrackingDatum>,
    redeemer: TrackingRedeemer,
    _utxo: OutputReference,
    tx: Transaction,
  ) {
    // The provided UTxO must have a TrackingDatum
    expect Some(tracking_datum) = datum

    // The redeemer must be ConsumeTracking
    let ConsumeTracking = redeemer

    // The tx must have exactly two outputs
    let has_two_outputs = list.length(tx.outputs) == 2

    // Find the ShipmentDatum output paid to outbox_address
    expect Some(shipment_output) =
      list.find(
        tx.outputs,
        fn(output) { output.address == tracking_datum.outbox_address },
      )

    expect InlineDatum(shipment_datum_data) = shipment_output.datum
    expect shipment_datum: ShipmentDatum = shipment_datum_data

    // Find the payment output
    expect Some(payment_output) =
      list.find(
        tx.outputs,
        fn(output) {
          output.address == address.from_verification_key(
            config.payment_address,
          )
        },
      )

    // Payment datum validations
    let payment_has_no_datum =
      when payment_output.datum is {
        InlineDatum(_) -> False
        _ -> True
      }

    // Shipment datum validations
    let valid_shipment_datum = and {
        shipment_datum.carrier == tracking_datum.carrier,
        shipment_datum.tracking_number == tracking_datum.tracking_number,
        is_valid_status(shipment_datum.status),
        list.has(tx.extra_signatories, shipment_datum.oracle_pkh),
      }

    // Value and address constraints
    let payment_receives_correct_amount =
      assets.lovelace_of(payment_output.value) >= config.tracking_price

    and {
      has_two_outputs,
      valid_shipment_datum,
      payment_receives_correct_amount,
      payment_has_no_datum,
    }
  }

  else(_ctx: ScriptContext) {
    fail @"unsupported purpose"
  }
}

fn is_valid_status(status: ByteArray) -> Bool {
  or {
    status == status_delivered,
    status == status_not_delivered,
  }
}
