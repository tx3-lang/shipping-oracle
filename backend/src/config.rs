use anyhow::{Context, Result, bail};
use std::env;

/// Application configuration loaded from environment variables
#[derive(Debug, Clone)]
pub struct Config {
    pub cron_schedule: String,
    pub shippo_api_key: String,
    pub validator_address: String,
    pub validator_script_ref: String,
    pub validator_script_hash: String,
    pub oracle_sk: String,
    pub oracle_pkh: String,
    pub oracle_payment_address: String,
    pub blockfrost_url: String,
    pub trp_url: String,
    pub trp_api_key: String,
}

impl Config {
    /// Load configuration from environment variables
    /// 
    /// # Environment Variables
    /// - `CRON_SCHEDULE`: Optional - Cron expression (default: "0 */5 * * * *")
    /// - `SHIPPO_API_KEY`: Required - Your Shippo API key
    /// - `VALIDATOR_ADDRESS`: Required - Cardano validator address
    /// - `VALIDATOR_SCRIPT_REF`: Required - Reference script UTXO (TxHash#TxIx)
    /// - `VALIDATOR_SCRIPT_HASH`: Required - Validator script hash (hex-encoded)
    /// - `ORACLE_SK`: Required - Oracle signing key (hex-encoded)
    /// - `ORACLE_PKH`: Required - Oracle public key (hex-encoded)
    /// - `ORACLE_PAYMENT_ADDRESS`: Required - Oracle payment address
    /// - `BLOCKFROST_URL`: Required - Blockfrost API URL
    /// - `TRP_URL`: Required - TRP API URL
    /// - `TRP_API_KEY`: Required - TRP API key
    pub fn from_env() -> Result<Self> {
        // Parse cron schedule (optional, has default)
        let cron_schedule = env::var("CRON_SCHEDULE")
            .unwrap_or_else(|_| "0 */5 * * * *".to_string());

        // Parse API key (required)
        let shippo_api_key = env::var("SHIPPO_API_KEY")
            .context("SHIPPO_API_KEY not set")?;
        
        if shippo_api_key.trim().is_empty() {
            bail!("SHIPPO_API_KEY cannot be empty");
        }

        // Parse validator address (required)
        let validator_address = env::var("VALIDATOR_ADDRESS")
            .context("VALIDATOR_ADDRESS not set")?;
        
        if validator_address.trim().is_empty() {
            bail!("VALIDATOR_ADDRESS cannot be empty");
        }

        // Parse validator script reference (required)
        let validator_script_ref = env::var("VALIDATOR_SCRIPT_REF")
            .context("VALIDATOR_SCRIPT_REF not set")?;
        
        if validator_script_ref.trim().is_empty() {
            bail!("VALIDATOR_SCRIPT_REF cannot be empty");
        }

        // Parse validator script hash (required)
        let validator_script_hash = env::var("VALIDATOR_SCRIPT_HASH")
            .context("VALIDATOR_SCRIPT_HASH not set")?;

        if validator_script_hash.trim().is_empty() {
            bail!("VALIDATOR_SCRIPT_HASH cannot be empty");
        }

        // Parse oracle signing key (required)
        let oracle_sk = env::var("ORACLE_SK")
            .context("ORACLE_SK not set")?;
        
        if oracle_sk.trim().is_empty() {
            bail!("ORACLE_SK cannot be empty");
        }

        // Parse oracle public key (required)
        let oracle_pkh = env::var("ORACLE_PKH")
            .context("ORACLE_PKH not set")?;
        
        if oracle_pkh.trim().is_empty() {
            bail!("ORACLE_PKH cannot be empty");
        }

        // Parse oracle payment address (required)
        let oracle_payment_address = env::var("ORACLE_PAYMENT_ADDRESS")
            .context("ORACLE_PAYMENT_ADDRESS not set")?;

        if oracle_payment_address.trim().is_empty() {
            bail!("ORACLE_PAYMENT_ADDRESS cannot be empty");
        }

        // Parse Blockfrost URL (required)
        let blockfrost_url = env::var("BLOCKFROST_URL")
            .context("BLOCKFROST_URL not set (required when OUTPUT_MODE is cardano)")?;
        
        if blockfrost_url.trim().is_empty() {
            bail!("BLOCKFROST_URL cannot be empty");
        }

        // Parse TRP URL (required)
        let trp_url = env::var("TRP_URL")
            .context("TRP_URL not set")?;
        
        if trp_url.trim().is_empty() {
            bail!("TRP_URL cannot be empty");
        }

        // Parse TRP API key (required)
        let trp_api_key = env::var("TRP_API_KEY")
            .context("TRP_API_KEY not set")?;
        
        if trp_api_key.trim().is_empty() {
            bail!("TRP_API_KEY cannot be empty");
        }

        Ok(Config {
            cron_schedule,
            shippo_api_key,
            validator_address,
            validator_script_ref,
            validator_script_hash,
            oracle_sk,
            oracle_pkh,
            oracle_payment_address,
            blockfrost_url,
            trp_url,
            trp_api_key,
        })
    }
}
