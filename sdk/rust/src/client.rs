use reqwest::Client;
use serde::de::DeserializeOwned;

use crate::error::OracleSdkError;
use crate::models::{HealthResponse, OracleAttestation, PreparedCommitment, ShipmentReference};
use crate::verify::{decode_array, verify_attestation};

#[derive(Debug, Clone)]
pub struct OracleClient {
    base_url: String,
    http_client: Client,
    expected_public_key: Option<[u8; 32]>,
}

impl OracleClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self::with_http_client(base_url, Client::new())
    }

    pub fn with_http_client(base_url: impl Into<String>, http_client: Client) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            http_client,
            expected_public_key: None,
        }
    }

    pub fn with_expected_public_key_hex(
        mut self,
        expected_public_key_hex: &str,
    ) -> Result<Self, OracleSdkError> {
        self.expected_public_key = Some(decode_array::<32>(
            "expected_public_key",
            expected_public_key_hex,
        )?);
        Ok(self)
    }

    pub async fn health(&self) -> Result<HealthResponse, OracleSdkError> {
        self.get_json::<HealthResponse>("/health", &[]).await
    }

    pub async fn fetch_attestation(
        &self,
        carrier: &str,
        tracking_number: &str,
    ) -> Result<OracleAttestation, OracleSdkError> {
        self.get_json(
            "/v1/shipment",
            &[("carrier", carrier), ("tracking_number", tracking_number)],
        )
        .await
    }

    pub async fn fetch_for(
        &self,
        shipment: &ShipmentReference,
    ) -> Result<OracleAttestation, OracleSdkError> {
        self.fetch_attestation(&shipment.carrier, &shipment.tracking_number)
            .await
    }

    pub async fn prepare_commitment<TContext>(
        &self,
        context: TContext,
        carrier: &str,
        tracking_number: &str,
    ) -> Result<PreparedCommitment<TContext>, OracleSdkError> {
        let attestation = self.fetch_attestation(carrier, tracking_number).await?;

        if attestation.plaintext.carrier != carrier {
            return Err(OracleSdkError::ResponseMismatch {
                field: "carrier",
                expected: carrier.to_string(),
                actual: attestation.plaintext.carrier.clone(),
            });
        }
        if attestation.plaintext.tracking_number != tracking_number {
            return Err(OracleSdkError::ResponseMismatch {
                field: "tracking_number",
                expected: tracking_number.to_string(),
                actual: attestation.plaintext.tracking_number.clone(),
            });
        }

        if let Some(expected_public_key) = self.expected_public_key.as_ref() {
            verify_attestation(&attestation, Some(expected_public_key))?;
        } else {
            attestation.verify()?;
        }

        Ok(PreparedCommitment::new(
            context,
            attestation,
            self.expected_public_key,
        ))
    }

    pub async fn prepare_for<TContext>(
        &self,
        context: TContext,
        shipment: &ShipmentReference,
    ) -> Result<PreparedCommitment<TContext>, OracleSdkError> {
        self.prepare_commitment(context, &shipment.carrier, &shipment.tracking_number)
            .await
    }

    async fn get_json<T>(&self, path: &str, query: &[(&str, &str)]) -> Result<T, OracleSdkError>
    where
        T: DeserializeOwned,
    {
        let response = self
            .http_client
            .get(format!("{}{}", self.base_url, path))
            .query(query)
            .send()
            .await?;

        if response.status().is_success() {
            return response.json::<T>().await.map_err(OracleSdkError::from);
        }

        let status = response.status().as_u16();
        let message = response.text().await.unwrap_or_default();
        Err(OracleSdkError::Api { status, message })
    }
}
