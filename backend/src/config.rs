use anyhow::{Context, Result, bail};
use std::env;

/// Application configuration loaded from environment variables.
///
/// # Required
/// - `SHIPPO_API_KEY`  — Shippo tracking API token
/// - `ORACLE_SK`       — Oracle Ed25519 signing key (32 bytes, hex)
/// - `ORACLE_PKH`      — Oracle verification key hash (28 bytes, hex)
/// - `ORACLE_ADDRESS`  — Cardano address the oracle controls
/// - `TRP_URL`         — TRP endpoint (used by tx3 consumers)
///
/// # Optional
/// - `LISTEN_ADDRESS`  — HTTP bind address (default `0.0.0.0:3000`)
/// - `TRP_API_KEY`     — TRP API key when the endpoint requires it
#[derive(Debug, Clone)]
pub struct Config {
    pub listen_address: String,
    pub shippo_api_key: String,
    pub oracle_sk: String,
    pub oracle_pkh: String,
    pub oracle_address: String,
    pub trp_url: String,
    pub trp_api_key: Option<String>,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let listen_address =
            env::var("LISTEN_ADDRESS").unwrap_or_else(|_| "0.0.0.0:3000".to_string());

        let shippo_api_key = require_env("SHIPPO_API_KEY")?;
        let oracle_sk = require_env("ORACLE_SK")?;
        let oracle_pkh = require_env("ORACLE_PKH")?;
        let oracle_address = require_env("ORACLE_ADDRESS")?;
        let trp_url = require_env("TRP_URL")?;

        let trp_api_key = match env::var("TRP_API_KEY") {
            Ok(v) if v.trim().is_empty() => bail!("TRP_API_KEY cannot be empty"),
            Ok(v) => Some(v),
            Err(_) => None,
        };

        Ok(Config {
            listen_address,
            shippo_api_key,
            oracle_sk,
            oracle_pkh,
            oracle_address,
            trp_url,
            trp_api_key,
        })
    }
}

fn require_env(key: &str) -> Result<String> {
    let value = env::var(key).with_context(|| format!("{key} not set"))?;
    if value.trim().is_empty() {
        bail!("{key} cannot be empty");
    }
    Ok(value)
}
