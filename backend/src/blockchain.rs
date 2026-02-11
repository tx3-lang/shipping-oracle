use anyhow::{Context, Ok, Result, anyhow};
use ed25519_dalek::{Signer, SigningKey};
use pallas::codec::{
    minicbor,
    utils::{Bytes, NonEmptySet, KeepRaw},
};
use pallas::ledger::{
    addresses::Address,
    primitives::{PlutusData, conway::VKeyWitness},
    traverse::MultiEraTx,
};
use reqwest::Client as HttpClient;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use tx3_sdk::trp::{ClientOptions, TxEnvelope};

use crate::config::Config;
use crate::models::{TrackingUTxO, TrackingDatum};
use crate::tx3::{Client as Tx3Client, CloseShipmentParams};

#[derive(Debug, Deserialize)]
struct BlockfrostTxSearch {
    tx_hash: String,
}

#[derive(Debug, Deserialize)]
struct BlockfrostTx {
    inputs: Vec<BlockfrostTxInput>,
    outputs: Vec<BlockfrostTxOutput>,
}

#[derive(Debug, Deserialize)]
struct BlockfrostTxInput {
    address: String,
    reference_script_hash: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BlockfrostTxOutput {
    address: String,
    output_index: u32,
    inline_datum: Option<String>,
}

impl TrackingDatum {
    pub fn from_cbor(datum_bytes: &str) -> Option<TrackingDatum> {
        let datum = minicbor::decode::<PlutusData>(
            &hex::decode(datum_bytes)
                .unwrap_or_default()
        );

        let mut carrier: Option<String> = None;
        let mut tracking_number: Option<String> = None;
        let mut outbox_address: Option<Address> = None;

        if datum.is_ok() {
            if let PlutusData::Constr(constr) = datum.unwrap() {
                let carrier_field = constr.fields.get(0);
                if let Some(PlutusData::BoundedBytes(carrier_bytes)) = carrier_field {
                    carrier = Some(String::from_utf8(carrier_bytes.to_vec()).unwrap_or_default());
                }

                let tracking_number_field = constr.fields.get(1);
                if let Some(PlutusData::BoundedBytes(tracking_number_bytes)) = tracking_number_field {
                    tracking_number = Some(String::from_utf8(tracking_number_bytes.to_vec()).unwrap_or_default());
                }

                let outbox_address_field = constr.fields.get(2);
                if let Some(PlutusData::BoundedBytes(outbox_address_bytes)) = outbox_address_field {
                    let address = Address::from_bytes(outbox_address_bytes);
                    if address.is_ok() {
                        outbox_address = Some(address.unwrap());
                    }
                }
            }
        }

        if !tracking_number.is_some() || !carrier.is_some() || !outbox_address.is_some() {
            return None
        }

        Some(TrackingDatum {
            carrier: carrier.unwrap(),
            tracking_number: tracking_number.unwrap(),
            outbox_address: outbox_address.unwrap(),
        })
    }
}

pub struct CardanoClient {
    config: Config,
    http_client: HttpClient,
    tx3_client: Tx3Client,
}

impl CardanoClient {
    pub fn new(config: Config) -> Result<Self> {
        let http_client = HttpClient::new();

        let mut headers = None;
        if let Some(trp_api_key) = &config.trp_api_key {
            headers = Some(HashMap::from([("dmtr-api-key".to_string(), trp_api_key.clone())]));
        }

        let tx3_client = Tx3Client::new(
            ClientOptions {
                endpoint: config.trp_url.clone(),
                headers,
            }
        );
        
        Ok(Self {
            config,
            http_client,
            tx3_client,
        })
    }

    async fn map_tx_to_tracking_utxo(&self, tx_hash: String) -> Option<TrackingUTxO> {
        let url = format!(
            "{}/txs/{}/utxos",
            self.config.blockfrost_url,
            tx_hash,
        );

        let response = self.http_client
            .get(&url)
            .send()
            .await;
        
        if response.is_err() || !response.as_ref().unwrap().status().is_success() {
            return None;
        }

        let tx: Result<BlockfrostTx, reqwest::Error> = response.unwrap().json().await;

        if tx.is_err() {
            return None;
        }

        let tx = tx.unwrap();

        if !tx.inputs.iter().any(|input| {
            input.address == self.config.oracle_address &&
            input.reference_script_hash.as_deref() == Some(&self.config.validator_script_hash)
        }) {
            return None;
        }

        let utxo = tx.outputs.iter().find(|output| {
            output.address == self.config.oracle_address &&
            output.inline_datum.is_some()
        });

        if utxo.is_none() {
            return None;
        }

        let utxo = utxo.unwrap();

        let datum = TrackingDatum::from_cbor(
            utxo.inline_datum.as_ref().unwrap()
        );

        if datum.is_none() {
            return None;
        }

        Some(TrackingUTxO {
            tx_hash: tx_hash.to_string(),
            tx_index: utxo.output_index,
            datum: datum.unwrap(),
        })
    }

    pub async fn fetch_shipments(&self) -> Result<Vec<TrackingUTxO>> {
        let url = format!(
            "{}/addresses/{}/transactions",
            self.config.blockfrost_url,
            self.config.oracle_address
        );
        
        let response = self.http_client
            .get(&url)
            .send()
            .await
            .context("Failed to query oracle transactions from Blockfrost")?;
        
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "Blockfrost query failed (status {}): {}",
                status,
                body
            ));
        }
        
        let txs: Vec<BlockfrostTxSearch> = response.json().await
            .context("Failed to parse Blockfrost transactions response")?;

        let shipments = futures::future::join_all(
            txs.into_iter()
            .map(|tx_search| {
                self.map_tx_to_tracking_utxo(tx_search.tx_hash.clone())
            })
        );
        
        Ok(shipments.await.into_iter().filter_map(|s| s).collect())
    }

    pub async fn submit_shipment(
        &self,
        tracking: &TrackingUTxO,
        status: &str,
    ) -> Result<String> {
        let envelope = self.tx3_client.close_shipment_tx(
            CloseShipmentParams {
                oracle: self.config.oracle_address.clone(),
                oracle_pkh: self.config.oracle_pkh.clone(),
                outbox: tracking.datum.outbox_address.to_string(),
                p_status: hex::encode(status.to_string()),
                p_timestamp: format!("{}", chrono::Utc::now().timestamp() as u64),
                p_utxo_ref: format!("{}#{}", tracking.tx_hash, tracking.tx_index),
                payment: self.config.oracle_payment_address.clone(),
                validator_script_ref: self.config.validator_script_ref.clone(),
            }
        ).await?;

        let cbor = self.sign_cbor(&envelope)?;

        let tx_hash = self.submit_transaction(cbor).await?;

        Ok(tx_hash)
    }

    fn sign_cbor(&self, envelope: &TxEnvelope) -> Result<Vec<u8>> {
        let tx_hash_bytes = hex::decode(&envelope.hash).expect("tx_hash must be hex");
        let private_key_bytes = hex::decode(&self.config.oracle_sk).expect("private_key must be hex");
        let signing_key = SigningKey::from_bytes(
            private_key_bytes
                .as_slice()
                .try_into()
                .expect("private_key must be 32 bytes"),
        );

        let signature = signing_key.sign(&tx_hash_bytes);
        let public_key = signing_key.verifying_key().to_bytes();

        let witness = VKeyWitness {
            vkey: Bytes::from(public_key.to_vec()),
            signature: Bytes::from(signature.to_bytes().to_vec()),
        };

        let bytes = hex::decode(&envelope.tx)?;
        let tx = MultiEraTx::decode(&bytes)?;
        let mut tx = tx.as_conway().ok_or(anyhow!("Unsupported tx era"))?.to_owned();

        let mut witness_set = tx.transaction_witness_set.unwrap();
        witness_set.vkeywitness = NonEmptySet::from_vec(vec![witness]);
        tx.transaction_witness_set = KeepRaw::from(witness_set);

        Ok(pallas::codec::minicbor::to_vec(&tx)?)
    }
    
    async fn submit_transaction(&self, signed_tx: Vec<u8>) -> Result<String> {
        let url = format!("{}/tx/submit", self.config.blockfrost_url);
        
        let response = self.http_client
            .post(&url)
            .header("Content-Type", "application/cbor")
            .body(signed_tx)
            .send()
            .await
            .context("Failed to submit transaction to Blockfrost")?;
        
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "Blockfrost transaction submission failed (status {}): {}",
                status,
                body
            ));
        }
        
        let response_json: Value = response.json().await
            .context("Failed to parse Blockfrost submission response")?;
        
        let tx_hash = response_json
            .as_str()
            .ok_or_else(|| anyhow!("Expected tx hash string in response"))?
            .to_string();
        
        Ok(tx_hash)
    }
}
