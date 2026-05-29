use aiken/builtin
use aiken/collection/list
use aiken/crypto
use cardano/assets
use cardano/transaction.{InlineDatum, Transaction}
use config
use types.{GovernanceDatum, OracleData}

pub fn verify(tx: Transaction, data: OracleData, signature: ByteArray) -> Bool {
  expect Some(gov_input) =
    list.find(
      tx.reference_inputs,
      fn(input) {
        assets.quantity_of(
          input.output.value,
          config.gov_policy_id,
          config.governance_asset_name,
        ) == 1
      },
    )
  expect InlineDatum(gov_data) = gov_input.output.datum
  expect gov: GovernanceDatum = gov_data
  let message = builtin.serialise_data(data)
  crypto.verify_ed25519_signature(gov.oracle_vk, message, signature)
}
