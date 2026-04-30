use anyhow::Result;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

use shipping_oracle::{
    api,
    config::Config,
    oracle_service::{OracleService, load_signing_key},
    shipment::ShipmentClient,
};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let config = Config::from_env()?;
    let shipment_client = ShipmentClient::new(&config)?;
    let signing_key = load_signing_key(&config.oracle_sk)?;
    let oracle_service = Arc::new(OracleService::new(shipment_client, signing_key));

    let app = api::create_router(oracle_service);
    let listener = TcpListener::bind(&config.listen_address).await?;
    tracing::info!(addr = %config.listen_address, "oracle listening");
    axum::serve(listener, app).await?;

    Ok(())
}
