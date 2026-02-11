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
            let shipment_response = self.shipment
                .fetch_shipment_status(
                    &shipment.datum.carrier,
                    &shipment.datum.tracking_number,
                )
                .await;

            if shipment_response.is_err() {
                println!("❌ Failed to fetch shipment status for {}/{}: {}", shipment.datum.carrier, shipment.datum.tracking_number, shipment_response.err().unwrap());
                continue;
            }

            let tracking_status = shipment_response.unwrap();

            println!("🔗 UTxO: {}#{}", shipment.tx_hash, shipment.tx_index);
            println!("🚚 Carrier: {}", shipment.datum.carrier);
            println!("📦 Tracking: {}", shipment.datum.tracking_number);
            println!("📍 Status: {} - {}", tracking_status.status, tracking_status.status_details);

            let status = get_status(&tracking_status);

            if status.is_some() {
                let submit_result = self.blockchain
                    .submit_shipment(
                        &shipment,
                        &status.unwrap(),
                    )
                    .await;

                if submit_result.is_err() {
                    println!("❌ Failed to submit transaction: {}", submit_result.err().unwrap());
                } else{
                    println!("✅ Submitted transaction: {}", submit_result.unwrap());
                }
            } else {
                println!("ℹ️  Status is not final, skipping update");
            }

            println!("================================");
        }

        Ok(())
    }
}
