use crate::blockchain::CardanoClient;
use crate::shipment::{ShipmentClient, get_status};
use std::sync::Arc;
    
pub struct DataFetcher {
    blockchain: Arc<CardanoClient>,
    shipment: Arc<ShipmentClient>,
}

impl DataFetcher {
    pub fn new(blockchain: Arc<CardanoClient>, shipment: Arc<ShipmentClient>) -> Self {
        Self { blockchain, shipment }
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let shipments = self.blockchain.fetch_shipments().await?;

        for shipment in shipments {
            let tracking_status = self.shipment
                .fetch_shipment_status(
                    &shipment.datum.carrier,
                    &shipment.datum.tracking_number,
                )
                .await?;

            println!("🔗 UTxO: {}#{}", shipment.tx_hash, shipment.tx_index);
            println!("🚚 Carrier: {}", shipment.datum.carrier);
            println!("📦 Tracking: {}", shipment.datum.tracking_number);
            println!("📍 Status: {} - {}", tracking_status.status, tracking_status.status_details);

            let status = get_status(&tracking_status);

            if status.is_some() {
                let tx_hash = self.blockchain
                    .submit_shipment(
                        &shipment,
                        &status.unwrap(),
                    )
                    .await?;

                println!("✅ Submitted transaction: {}", tx_hash);
            } else {
                println!("ℹ️  Status is not final, skipping update");
            }

            println!("================================");
        }

        Ok(())
    }
}
