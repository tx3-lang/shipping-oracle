use anyhow::Result;
use std::sync::Arc;
use shipping_oracle::{
    scheduler,
    config::Config,
    fetcher::DataFetcher,
    shipment::ShipmentClient,
    blockchain::CardanoClient,
};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    let config = match Config::from_env() {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Configuration error: {}", e);
            std::process::exit(1);
        }
    };
    
    let data_handler = Arc::new(DataFetcher::new(
        Arc::new(CardanoClient::new(config.clone())?),
        Arc::new(ShipmentClient::new(config.clone())?),
    ));

    println!("Cron schedule: {}", config.cron_schedule);
    println!("================================");
    
    scheduler::create_and_run_scheduler(config, data_handler).await?;
    
    Ok(())
}
