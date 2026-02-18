use anyhow::{Result, anyhow};
use pallas::ledger::addresses::Address;
use serde::Serialize;
use std::fs;
use std::sync::{Arc, Mutex};

use shipping_oracle::blockchain::CardanoClient;
use shipping_oracle::config::Config;
use shipping_oracle::models::{TrackingDatum, TrackingUTxO};
use shipping_oracle::shipment::{ShipmentClient, get_status};
use shipping_oracle::submitter::TxSubmitter;

const OUTBOX_ADDRESS: &str = "addr_test1qqcytargera54zzzgk9ajg2y2xlhrx4efgvjfe970vr57cxkxjyj4nx7n47t6s9saftdn3dypt4573lawvqutsh2ydrs3hxqj3";
const ORACLE_ADDRESS: &str = "addr_test1vqpp4rqsgkhyaz5ejjtwzane9wnkggfrn9pptgmtwq7fqws6t8yck";
const ORACLE_PKH: &str = "021a8c1045ae4e8a999496e176792ba7642123994215a36b703c903a";
const VALIDATOR_SCRIPT_REF: &str = "a6a57fe7cfcd69537dc88bfe4321cd7f164f26afd21c91c78cced224e6496f41#1";
const ORACLE_PAYMENT_ADDRESS: &str = "addr_test1vqpp4rqsgkhyaz5ejjtwzane9wnkggfrn9pptgmtwq7fqws6t8yck";

const DELIVERED_TIMESTAMP: u64 = 1771090081;
const FAILURE_TIMESTAMP: u64 = 1771090081;

const DELIVERED_HASH: &str = "584cbabb4a075d96d065b6e158d737f98c961dc5802e4b3f905f1f533d28f68f";
const FAILURE_HASH: &str = "bd97a5069c098f9402f889dc74b0c808fb3996c80a248eae9a15ba9b020a4e7e";

const DELIVERED_UTXO: &str = "a7a264ac1bef1f0da6312d16fb8f68b18d1a0d5010d8cb6a61790bf02a759301#0";
const FAILURE_UTXO: &str = "8185f0c4c844e28214c52c6871753304f273d0fea872b95fe6e32974d16ef520#0";
const TRANSIT_UTXO: &str = "59d035914f8e34d2c69520b110b3354e8519c4c2a12872d1204156953c18a861#0";

const DELIVERED_TRACKING: &str = "SHIPPO_DELIVERED";
const FAILURE_TRACKING: &str = "SHIPPO_FAILURE";
const TRANSIT_TRACKING: &str = "SHIPPO_TRANSIT";

const SHIPPO_CARRIER: &str = "shippo";

#[derive(Clone)]
struct MockSubmitter {
    expected_hash: String,
    calls: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl MockSubmitter {
    fn new(expected_hash: &str, calls: Arc<Mutex<Vec<Vec<u8>>>>) -> Self {
        Self {
            expected_hash: expected_hash.to_string(),
            calls,
        }
    }
}

#[async_trait::async_trait]
impl TxSubmitter for MockSubmitter {
    async fn submit(&self, signed_tx: Vec<u8>) -> Result<String> {
        self.calls.lock().map_err(|_| anyhow!("submit lock poisoned"))?.push(signed_tx);
        Ok(self.expected_hash.clone())
    }
}

#[derive(Serialize, Clone)]
struct CaseReport {
    name: String,
    tracking_utxo: String,
    tracking_tx_hash: String,
    tracking_tx_index: u32,
    tracking_outbox: String,
    carrier: String,
    tracking_number: String,
    expected_status: String,
    actual_status: Option<String>,
    status_details: Option<String>,
    derived_status: Option<String>,
    expected_timestamp: Option<u64>,
    expected_tx_hash: Option<String>,
    actual_tx_hash: Option<String>,
    expected_outbox: Option<String>,
    actual_outbox: Option<String>,
    expected_p_status: Option<String>,
    actual_p_status: Option<String>,
    expected_p_utxo_ref: Option<String>,
    actual_p_utxo_ref: Option<String>,
    expected_oracle: Option<String>,
    actual_oracle: Option<String>,
    expected_oracle_pkh: Option<String>,
    actual_oracle_pkh: Option<String>,
    expected_payment: Option<String>,
    actual_payment: Option<String>,
    expected_validator_script_ref: Option<String>,
    actual_validator_script_ref: Option<String>,
    passed: bool,
    errors: Vec<String>,
}

#[derive(Serialize)]
struct Report {
    cases: Vec<CaseReport>,
    passed: usize,
    failed: usize,
}

#[tokio::test]
async fn integration_tracking_to_shipment() -> Result<()> {
    dotenvy::dotenv().ok();
    
    let config = Config::from_env()?;
    let shipment_client = ShipmentClient::new(config.clone())?;

    let mut cases = Vec::new();

    cases.push(run_transit_case(&shipment_client).await?);
    cases.push(run_close_case(
        &config,
        &shipment_client,
        "delivered",
        DELIVERED_UTXO,
        DELIVERED_TRACKING,
        "DELIVERED",
        "DELIVERED",
        "44454c495645524544",
        DELIVERED_TIMESTAMP,
        DELIVERED_HASH,
    ).await?);
    cases.push(run_close_case(
        &config,
        &shipment_client,
        "failure",
        FAILURE_UTXO,
        FAILURE_TRACKING,
        "FAILURE",
        "NOT_DELIVERED",
        "4e4f545f44454c495645524544",
        FAILURE_TIMESTAMP,
        FAILURE_HASH,
    ).await?);

    write_reports(&cases)?;

    let failed = cases.iter().filter(|case| !case.passed).count();
    if failed > 0 {
        return Err(anyhow!("{} integration case(s) failed", failed));
    }

    Ok(())
}

async fn run_transit_case(shipment_client: &ShipmentClient) -> Result<CaseReport> {
    let mut errors = Vec::new();
    let status = shipment_client
        .fetch_shipment_status(SHIPPO_CARRIER, TRANSIT_TRACKING)
        .await;

    let (actual_status, status_details, derived_status) = match status {
        Ok(status) => {
            if status.status != "TRANSIT" {
                errors.push(format!("expected status TRANSIT, got {}", status.status));
            }

            let derived = get_status(&status);
            if derived.is_some() {
                errors.push("expected non-final status to be skipped".to_string());
            }

            (Some(status.status), Some(status.status_details), derived)
        }
        Err(err) => {
            errors.push(format!("failed to fetch status: {}", err));
            (None, None, None)
        }
    };

    let (tracking_tx_hash, tracking_tx_index) = split_utxo(TRANSIT_UTXO)?;

    Ok(CaseReport {
        name: "transit_skip".to_string(),
        tracking_utxo: TRANSIT_UTXO.to_string(),
        tracking_tx_hash,
        tracking_tx_index,
        tracking_outbox: OUTBOX_ADDRESS.to_string(),
        carrier: SHIPPO_CARRIER.to_string(),
        tracking_number: TRANSIT_TRACKING.to_string(),
        expected_status: "TRANSIT".to_string(),
        actual_status,
        status_details,
        derived_status,
        expected_timestamp: None,
        expected_tx_hash: None,
        actual_tx_hash: None,
        expected_outbox: None,
        actual_outbox: None,
        expected_p_status: None,
        actual_p_status: None,
        expected_p_utxo_ref: None,
        actual_p_utxo_ref: None,
        expected_oracle: None,
        actual_oracle: None,
        expected_oracle_pkh: None,
        actual_oracle_pkh: None,
        expected_payment: None,
        actual_payment: None,
        expected_validator_script_ref: None,
        actual_validator_script_ref: None,
        passed: errors.is_empty(),
        errors,
    })
}

async fn run_close_case(
    config: &Config,
    shipment_client: &ShipmentClient,
    name: &str,
    utxo_ref: &str,
    tracking_number: &str,
    expected_status: &str,
    expected_derived_status: &str,
    expected_p_status: &str,
    timestamp: u64,
    expected_hash: &str,
) -> Result<CaseReport> {
    let mut errors = Vec::new();

    let status = shipment_client
        .fetch_shipment_status(SHIPPO_CARRIER, tracking_number)
        .await;

    let (actual_status, status_details, derived_status) = match status {
        Ok(status) => {
            if status.status != expected_status {
                errors.push(format!("expected status {}, got {}", expected_status, status.status));
            }
            let derived = get_status(&status);
            if let Some(ref derived_status) = derived {
                if derived_status != expected_derived_status {
                    errors.push(format!(
                        "expected derived status {}, got {}",
                        expected_derived_status,
                        derived_status
                    ));
                }
            }
            (Some(status.status), Some(status.status_details), derived)
        }
        Err(err) => {
            errors.push(format!("failed to fetch status: {}", err));
            (None, None, None)
        }
    };

    let (tx_hash, params, envelope_hash, submit_calls) = if errors.is_empty() {
        let outbox_address = Address::from_bech32(OUTBOX_ADDRESS)
            .map_err(|err| anyhow!("invalid outbox address: {}", err))?;
        let (tx_hash, tx_index) = split_utxo(utxo_ref)?;
        let tracking = TrackingUTxO {
            tx_hash,
            tx_index,
            datum: TrackingDatum {
                carrier: SHIPPO_CARRIER.to_string(),
                tracking_number: tracking_number.to_string(),
                outbox_address,
            },
        };

        let calls = Arc::new(Mutex::new(Vec::new()));
        let submitter = MockSubmitter::new(expected_hash, calls.clone());
        let client = CardanoClient::with_submitter(config.clone(), Box::new(submitter))?;

        let derived_status_value = derived_status.clone().unwrap_or_default();
        if derived_status_value.is_empty() {
            errors.push("expected a final status to submit".to_string());
            (None, None, None, 0)
        } else {
            let (params, envelope) = client
                .prepare_close_shipment_at(&tracking, &derived_status_value, timestamp)
                .await?;

            let submit_result = client
                .submit_shipment_at(&tracking, &derived_status_value, timestamp)
                .await;

            let submit_calls = calls.lock().map_err(|_| anyhow!("submit lock poisoned"))?.len();
            let tx_hash = submit_result.ok();

            (tx_hash, Some(params), Some(envelope.hash), submit_calls)
        }
    } else {
        (None, None, None, 0)
    };

    if let Some(ref envelope_hash) = envelope_hash {
        if envelope_hash != expected_hash {
            errors.push(format!("expected envelope hash {}, got {}", expected_hash, envelope_hash));
        }
    }

    if submit_calls != 1 {
        errors.push(format!("expected 1 submit call, got {}", submit_calls));
    }

    if let Some(ref tx_hash) = tx_hash {
        if tx_hash != expected_hash {
            errors.push(format!("expected tx hash {}, got {}", expected_hash, tx_hash));
        }
    } else {
        errors.push("missing tx hash from submission".to_string());
    }

    let (actual_outbox, actual_p_status, actual_p_utxo_ref, actual_oracle, actual_oracle_pkh, actual_payment, actual_validator_script_ref) = if let Some(ref params) = params {
        if !is_numeric(&params.p_timestamp) {
            errors.push("expected numeric p_timestamp".to_string());
        }

        if params.p_status != expected_p_status {
            errors.push(format!("expected p_status {}, got {}", expected_p_status, params.p_status));
        }

        if params.p_utxo_ref != utxo_ref {
            errors.push(format!("expected p_utxo_ref {}, got {}", utxo_ref, params.p_utxo_ref));
        }

        if params.outbox != OUTBOX_ADDRESS {
            errors.push(format!("expected outbox {}, got {}", OUTBOX_ADDRESS, params.outbox));
        }

        if params.oracle != ORACLE_ADDRESS {
            errors.push(format!("expected oracle {}, got {}", ORACLE_ADDRESS, params.oracle));
        }

        if params.oracle_pkh != ORACLE_PKH {
            errors.push(format!("expected oracle_pkh {}, got {}", ORACLE_PKH, params.oracle_pkh));
        }

        if params.payment != ORACLE_PAYMENT_ADDRESS {
            errors.push(format!("expected payment {}, got {}", ORACLE_PAYMENT_ADDRESS, params.payment));
        }

        if params.validator_script_ref != VALIDATOR_SCRIPT_REF {
            errors.push(format!(
                "expected validator_script_ref {}, got {}",
                VALIDATOR_SCRIPT_REF,
                params.validator_script_ref
            ));
        }

        (
            Some(params.outbox.clone()),
            Some(params.p_status.clone()),
            Some(params.p_utxo_ref.clone()),
            Some(params.oracle.clone()),
            Some(params.oracle_pkh.clone()),
            Some(params.payment.clone()),
            Some(params.validator_script_ref.clone()),
        )
    } else {
        (None, None, None, None, None, None, None)
    };

    let (tracking_tx_hash, tracking_tx_index) = split_utxo(utxo_ref)?;

    Ok(CaseReport {
        name: name.to_string(),
        tracking_utxo: utxo_ref.to_string(),
        tracking_tx_hash,
        tracking_tx_index,
        tracking_outbox: OUTBOX_ADDRESS.to_string(),
        carrier: SHIPPO_CARRIER.to_string(),
        tracking_number: tracking_number.to_string(),
        expected_status: expected_status.to_string(),
        actual_status,
        status_details,
        derived_status,
        expected_timestamp: Some(timestamp),
        expected_tx_hash: Some(expected_hash.to_string()),
        actual_tx_hash: tx_hash,
        expected_outbox: Some(OUTBOX_ADDRESS.to_string()),
        actual_outbox,
        expected_p_status: Some(expected_p_status.to_string()),
        actual_p_status,
        expected_p_utxo_ref: Some(utxo_ref.to_string()),
        actual_p_utxo_ref,
        expected_oracle: Some(ORACLE_ADDRESS.to_string()),
        actual_oracle,
        expected_oracle_pkh: Some(ORACLE_PKH.to_string()),
        actual_oracle_pkh,
        expected_payment: Some(ORACLE_PAYMENT_ADDRESS.to_string()),
        actual_payment,
        expected_validator_script_ref: Some(VALIDATOR_SCRIPT_REF.to_string()),
        actual_validator_script_ref,
        passed: errors.is_empty(),
        errors,
    })
}

fn split_utxo(utxo_ref: &str) -> Result<(String, u32)> {
    let mut parts = utxo_ref.split('#');
    let tx_hash = parts.next().ok_or_else(|| anyhow!("missing tx hash"))?;
    let index = parts.next().ok_or_else(|| anyhow!("missing tx index"))?;
    if parts.next().is_some() {
        return Err(anyhow!("invalid utxo ref"));
    }

    let tx_index = index.parse::<u32>()
        .map_err(|_| anyhow!("invalid tx index"))?;
    Ok((tx_hash.to_string(), tx_index))
}

fn is_numeric(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit())
}

fn write_reports(cases: &[CaseReport]) -> Result<()> {
    let reports_dir = std::path::Path::new("reports");
    fs::create_dir_all(reports_dir)?;

    let passed = cases.iter().filter(|case| case.passed).count();
    let failed = cases.len() - passed;
    let report = Report {
        cases: cases.to_vec(),
        passed,
        failed,
    };

    let json_path = reports_dir.join("integration.json");
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(&json_path, json)?;

    let md_path = reports_dir.join("integration.md");
    let md = render_markdown(&report);
    fs::write(&md_path, md)?;

    Ok(())
}

fn render_markdown(report: &Report) -> String {
    let mut out = String::new();
    out.push_str("# Integration Test Report\n\n");
    out.push_str(&format!("- Passed: {}\n", report.passed));
    out.push_str(&format!("- Failed: {}\n\n", report.failed));

    for case in &report.cases {
        let title = match case.name.as_str() {
            "transit_skip" => "(transit_skip) No transition test",
            "delivered" => "(delivered) Delivered transition test",
            "failure" => "(failure) Not delivered transition test",
            _ => case.name.as_str(),
        };

        out.push_str(&format!("## {}\n", title));
        out.push_str("### Tracking UTxO\n");
        let tracking_json = serde_json::json!({
            "tx_hash": case.tracking_tx_hash,
            "tx_index": case.tracking_tx_index,
            "datum": {
                "carrier": case.carrier,
                "tracking_number": case.tracking_number,
                "outbox_address": case.tracking_outbox,
            }
        });
        out.push_str("```\n");
        out.push_str(&serde_json::to_string_pretty(&tracking_json).unwrap_or_else(|_| "{}".to_string()));
        out.push_str("\n```\n");

        out.push_str("### Shipment\n");
        out.push_str(&format!("- Carrier: {}\n", case.carrier));
        out.push_str(&format!("- Tracking: {}\n", case.tracking_number));
        if let Some(ref actual_status) = case.actual_status {
            out.push_str(&format!("- Status: {}\n", actual_status));
        } else {
            out.push_str(&format!("- Status: {}\n", case.expected_status));
        }
        if let Some(ref status_details) = case.status_details {
            out.push_str(&format!("- Details: {}\n", status_details));
        }

        out.push_str("### Transition\n");
        let transition_to = match case.derived_status.as_deref() {
            Some(status) => status,
            None => "NO TRANSITION",
        };
        out.push_str("```\n");
        out.push_str(&format!("{} -> {}\n", case.expected_status, transition_to));
        out.push_str("```\n");

        if let Some(ref tx_hash) = case.expected_tx_hash {
            out.push_str("### Shipment UTxO\n");
            let shipment_json = serde_json::json!({
                "tx_hash": tx_hash,
                "tx_index": 0,
                "datum": {
                    "carrier": case.carrier,
                    "tracking_number": case.tracking_number,
                    "status": case.derived_status.clone().unwrap_or_else(|| "UNKNOWN".to_string()),
                    "timestamp": case.expected_timestamp,
                    "oracle_pkh": case.expected_oracle_pkh,
                }
            });
            out.push_str("```\n");
            out.push_str(&serde_json::to_string_pretty(&shipment_json).unwrap_or_else(|_| "{}".to_string()));
            out.push_str("\n```\n");
        }

        out.push_str(&format!("### Result -> `{}`\n", if case.passed { "PASS" } else { "FAIL" }));
        if !case.errors.is_empty() {
            out.push_str(&format!("- Errors: {}\n", case.errors.join("; ")));
        }
        out.push('\n');
    }

    out
}
