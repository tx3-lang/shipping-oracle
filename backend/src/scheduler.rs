use anyhow::Result;
use tokio_cron_scheduler::{Job, JobScheduler};
use std::sync::Arc;
use crate::{
    config::Config,
    fetcher::DataFetcher,
};

pub async fn create_and_run_scheduler(config: Config, data_fetcher: Arc<DataFetcher>) -> Result<()> {
    let scheduler = JobScheduler::new().await?;

    let job_data_fetcher = data_fetcher.clone();
    let job = Job::new_async(config.cron_schedule.as_str(), move |_uuid, _l| {
        let data_fetcher = job_data_fetcher.clone();
        Box::pin(async move {
            execute_fetch_job(data_fetcher).await;
        })
    })?;

    scheduler.add(job).await?;
    scheduler.start().await?;

    execute_fetch_job(data_fetcher.clone()).await;

    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
    }
}

async fn execute_fetch_job(data_fetcher: Arc<DataFetcher>) {
    println!(
        "[{}] Executing scheduled fetch...",
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
    );
    println!("================================");

    if let Err(e) = data_fetcher.run().await {
        eprintln!("Error during fetch job: {:?}", e);
    } else {
        println!(
            "[{}] Fetch job completed successfully",
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
        );
    }
    println!("================================");
}
