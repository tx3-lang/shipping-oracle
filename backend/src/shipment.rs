use anyhow::{Context, Result};
use reqwest::Client;

use crate::config::Config;
use crate::models::{TrackingResponse, TrackingStatus};

pub struct ShipmentClient {
    config: Config,
    http_client: Client,
}

impl ShipmentClient {
    pub fn new(config: Config) -> Result<Self> {
        let http_client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .context("Failed to create HTTP client")?;
        
        Ok(Self { config, http_client })
    }

    pub async fn fetch_shipment_status(&self, carrier: &str, tracking_number: &str) -> Result<TrackingStatus> {
        let url = format!(
            "https://api.goshippo.com/tracks/{}/{}",
            carrier,
            tracking_number
        );

        let response = self.http_client
            .get(&url)
            .header("Authorization", format!("ShippoToken {}", self.config.shippo_api_key))
            .send()
            .await
            .context("Failed to send request to Shipment API")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!(
                "Shipment API query failed (status {}): {}",
                status,
                body
            );
        }

        let tracking: TrackingResponse = response
            .json()
            .await
            .context("Failed to parse Shipment API response")?;

        Ok(tracking.tracking_status)
    }
}

pub fn get_status(tracking_status: &TrackingStatus) -> Option<String> {
    match tracking_status.status.as_str() {
        "DELIVERED" => Some("DELIVERED".to_string()),
        "RETURNED" => Some("NOT_DELIVERED".to_string()),
        "FAILURE" => Some("NOT_DELIVERED".to_string()),
        _ => None,
    }
}
