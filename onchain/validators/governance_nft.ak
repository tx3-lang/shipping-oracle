use aiken/collection/dict
use aiken/collection/list
use cardano/address.{Address, VerificationKey}
use cardano/assets
use cardano/transaction.{Input, NoDatum, Output, OutputReference, Transaction}
use config

// One-shot minting policy. Guarantees NFT uniqueness by requiring consumption
// of a specific seed UTxO (hardcoded via config). Once the seed UTxO is spent
// it can never be spent again, so this policy can only mint once.
//
// Validation rules:
//   1. A tx input must reference the configured seed_utxo.
//   2. Exactly one token is minted under this policy, with the configured
//      asset_name and quantity 1.

validator governance_nft {
  mint(_redeemer: Data, policy_id: ByteArray, tx: Transaction) {
    let seed_ref =
      OutputReference {
        transaction_id: config.seed_utxo_tx_hash,
        output_index: config.seed_utxo_index,
      }
    let consumes_seed =
      list.any(tx.inputs, fn(input) { input.output_reference == seed_ref })
    let minted = tx.mint |> assets.tokens(policy_id) |> dict.to_pairs
    when minted is {
      [Pair(asset_name, 1)] ->
        consumes_seed && asset_name == config.governance_asset_name
      _ -> False
    }
  }

  else(_) {
    fail @"unsupported purpose"
  }
}

// ---- tests ----

const test_policy_id: ByteArray =
  #"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"

fn dummy_address() -> Address {
  Address {
    payment_credential: VerificationKey(
      #"beefbeefbeefbeefbeefbeefbeefbeefbeefbeefbeefbeefbeefbeef",
    ),
    stake_credential: None,
  }
}

fn dummy_input(ref: OutputReference) -> Input {
  Input {
    output_reference: ref,
    output: Output {
      address: dummy_address(),
      value: assets.from_lovelace(2_000_000),
      datum: NoDatum,
      reference_script: None,
    },
  }
}

fn seed_input() -> Input {
  dummy_input(
    OutputReference {
      transaction_id: config.seed_utxo_tx_hash,
      output_index: config.seed_utxo_index,
    },
  )
}

fn other_input() -> Input {
  dummy_input(
    OutputReference {
      transaction_id: #"1111111111111111111111111111111111111111111111111111111111111111",
      output_index: 7,
    },
  )
}

test mint_valid() {
  let tx =
    Transaction {
      ..transaction.placeholder,
      inputs: [seed_input()],
      mint: assets.from_asset(test_policy_id, config.governance_asset_name, 1),
    }
  governance_nft.mint(Void, test_policy_id, tx)
}

test mint_missing_seed_input() fail {
  let tx =
    Transaction {
      ..transaction.placeholder,
      inputs: [other_input()],
      mint: assets.from_asset(test_policy_id, config.governance_asset_name, 1),
    }
  governance_nft.mint(Void, test_policy_id, tx)
}

test mint_wrong_asset_name() fail {
  let tx =
    Transaction {
      ..transaction.placeholder,
      inputs: [seed_input()],
      mint: assets.from_asset(test_policy_id, #"deadbeef", 1),
    }
  governance_nft.mint(Void, test_policy_id, tx)
}

test mint_wrong_quantity() fail {
  let tx =
    Transaction {
      ..transaction.placeholder,
      inputs: [seed_input()],
      mint: assets.from_asset(test_policy_id, config.governance_asset_name, 2),
    }
  governance_nft.mint(Void, test_policy_id, tx)
}

test mint_extra_asset() fail {
  let tx =
    Transaction {
      ..transaction.placeholder,
      inputs: [seed_input()],
      mint: assets.merge(
        assets.from_asset(test_policy_id, config.governance_asset_name, 1),
        assets.from_asset(test_policy_id, #"deadbeef", 1),
      ),
    }
  governance_nft.mint(Void, test_policy_id, tx)
}
