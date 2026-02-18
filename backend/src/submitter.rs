use anyhow::{Context, Result, anyhow};
use reqwest::Client as HttpClient;
use serde_json::Value;

#[async_trait::async_trait]
pub trait TxSubmitter: Send + Sync {
    async fn submit(&self, signed_tx: Vec<u8>) -> Result<String>;
}

pub struct BlockfrostSubmitter {
    blockfrost_url: String,
    http_client: HttpClient,
}

impl BlockfrostSubmitter {
    pub fn new(blockfrost_url: String, http_client: HttpClient) -> Self {
        Self {
            blockfrost_url,
            http_client,
        }
    }
}

#[async_trait::async_trait]
impl TxSubmitter for BlockfrostSubmitter {
    async fn submit(&self, signed_tx: Vec<u8>) -> Result<String> {
        let url = format!("{}/tx/submit", self.blockfrost_url);

        let response = self
            .http_client
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

        let response_json: Value = response
            .json()
            .await
            .context("Failed to parse Blockfrost submission response")?;

        let tx_hash = response_json
            .as_str()
            .ok_or_else(|| anyhow!("Expected tx hash string in response"))?
            .to_string();

        Ok(tx_hash)
    }
}
