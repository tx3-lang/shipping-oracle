use anyhow::{Context, Result};
use reqwest::Client;

use crate::config::Config;
use crate::models::{TrackingResponse, TrackingStatus};

pub struct ShipmentClient {
    shippo_api_key: String,
    base_url: String,
    http_client: Client,
}

impl ShipmentClient {
    pub fn new(config: &Config) -> Result<Self> {
        Self::with_base_url(config, "https://api.goshippo.com".to_string())
    }

    pub fn with_base_url(config: &Config, base_url: String) -> Result<Self> {
        let http_client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .context("failed to create HTTP client")?;

        Ok(Self {
            shippo_api_key: config.shippo_api_key.clone(),
            base_url,
            http_client,
        })
    }

    pub async fn fetch_shipment_status(
        &self,
        carrier: &str,
        tracking_number: &str,
    ) -> Result<TrackingStatus> {
        let url = format!("{}/tracks/{carrier}/{tracking_number}", self.base_url);

        let response = self
            .http_client
            .get(&url)
            .header("Authorization", format!("ShippoToken {}", self.shippo_api_key))
            .send()
            .await
            .context("failed to send request to Shippo")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Shippo query failed (status {status}): {body}");
        }

        let tracking: TrackingResponse =
            response.json().await.context("failed to parse Shippo response")?;

        Ok(tracking.tracking_status)
    }
}

/// Map Shippo's status vocabulary to the one we expose on-chain.
///
/// Shippo docs: https://docs.goshippo.com/docs/tracking/statuses
pub fn normalize_status(tracking_status: &TrackingStatus) -> String {
    match tracking_status.status.as_str() {
        "DELIVERED" => "DELIVERED",
        "FAILURE" | "RETURNED" => "NOT_DELIVERED",
        "TRANSIT" => "IN_TRANSIT",
        "PRE_TRANSIT" => "PRE_TRANSIT",
        _ => "UNKNOWN",
    }
    .to_string()
}
