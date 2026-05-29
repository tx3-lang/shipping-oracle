mod common;

use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use serde::Serialize;
use shipping_oracle_sdk::{OracleClient, OracleSdkError, OracleStatus, ShipmentReference};

use common::{
    DELIVERED_TRACKING, ERROR_TRACKING, PRE_TRANSIT_TRACKING, RETURNED_TRACKING, SHIPPO_CARRIER,
    TEST_PUBLIC_KEY_HEX, TRANSIT_TRACKING, UNKNOWN_TRACKING, start_mocked_oracle,
    stub_shippo_error, stub_shippo_status,
};

#[derive(Serialize, Default)]
struct CaseReport {
    name: String,
    tracking_number: String,
    expected_status: String,
    actual_status: Option<String>,
    http_result: String,
    signature_verified: bool,
    tx3_args_generated: bool,
    tx3_args_json: Option<String>,
    passed: bool,
    errors: Vec<String>,
}

#[derive(Serialize)]
struct Report {
    generated_at: String,
    sdk_version: String,
    passed: usize,
    failed: usize,
    cases: Vec<CaseReport>,
}

async fn success_case(
    client: &OracleClient,
    name: &str,
    tracking_number: &str,
    expected_status: OracleStatus,
) -> CaseReport {
    let mut case = CaseReport {
        name: name.to_string(),
        tracking_number: tracking_number.to_string(),
        expected_status: expected_status.to_string(),
        ..Default::default()
    };

    let shipment = ShipmentReference::new(SHIPPO_CARRIER, tracking_number);
    let commitment = match client.prepare_for(name.to_string(), &shipment).await {
        Ok(commitment) => commitment,
        Err(err) => {
            case.http_result = format!("error: {err}");
            case.errors.push(format!("prepare commitment: {err}"));
            return case;
        }
    };

    case.http_result = "ok".to_string();
    case.actual_status = Some(commitment.attestation.data.status.to_string());

    match commitment.verify() {
        Ok(_) => case.signature_verified = true,
        Err(err) => case.errors.push(format!("verify: {err}")),
    }

    match commitment.to_cli_args_json() {
        Ok(args) => {
            case.tx3_args_generated = true;
            case.tx3_args_json = Some(args.as_json_string());
        }
        Err(err) => case.errors.push(format!("tx3 args: {err}")),
    }

    if case.actual_status.as_deref() != Some(expected_status.as_str()) {
        case.errors.push(format!(
            "status mismatch: expected {}, got {}",
            expected_status,
            case.actual_status.as_deref().unwrap_or("<none>")
        ));
    }

    case.passed = case.errors.is_empty();
    case
}

async fn error_case(client: &OracleClient, name: &str, tracking_number: &str) -> CaseReport {
    let mut case = CaseReport {
        name: name.to_string(),
        tracking_number: tracking_number.to_string(),
        expected_status: "http 502".to_string(),
        ..Default::default()
    };

    let err = client
        .fetch_attestation(SHIPPO_CARRIER, tracking_number)
        .await
        .expect_err("upstream error should surface");

    case.http_result = err.to_string();
    match err {
        OracleSdkError::Api { status: 502, .. } => case.passed = true,
        other => case
            .errors
            .push(format!("expected 502 api error, got {other}")),
    }

    case
}

fn render_markdown(report: &Report) -> String {
    let mut out = String::new();
    out.push_str("# Shipping Oracle Rust SDK Report\n\n");
    out.push_str(&format!("- **Generated:** `{}`\n", report.generated_at));
    out.push_str(&format!("- **SDK version:** `{}`\n", report.sdk_version));
    out.push_str(&format!(
        "- **Result:** {} passed / {} failed (out of {})\n\n",
        report.passed,
        report.failed,
        report.cases.len()
    ));
    out.push_str("## Cases\n\n");
    out.push_str("| # | Case | Tracking | Expected | Actual | Verify | Tx3 Args | Result |\n");
    out.push_str("|---|---|---|---|---|---|---|---|\n");
    for (index, case) in report.cases.iter().enumerate() {
        out.push_str(&format!(
            "| {} | {} | `{}` | `{}` | `{}` | {} | {} | {} |\n",
            index + 1,
            case.name,
            case.tracking_number,
            case.expected_status,
            case.actual_status.as_deref().unwrap_or("-"),
            if case.signature_verified { "✅" } else { "-" },
            if case.tx3_args_generated { "✅" } else { "-" },
            if case.passed { "✅ PASS" } else { "❌ FAIL" },
        ));
    }
    out.push_str("\n## Errors\n\n");
    let mut had_errors = false;
    for case in &report.cases {
        if !case.errors.is_empty() {
            had_errors = true;
            out.push_str(&format!("- **{}**\n", case.name));
            for error in &case.errors {
                out.push_str(&format!("  - {}\n", error));
            }
        }
    }
    if !had_errors {
        out.push_str("(none)\n");
    }
    out
}

#[tokio::test]
async fn generate_sdk_integration_report() -> Result<()> {
    let (shippo, oracle) = start_mocked_oracle().await;
    stub_shippo_status(&shippo, DELIVERED_TRACKING, "DELIVERED").await;
    stub_shippo_status(&shippo, TRANSIT_TRACKING, "TRANSIT").await;
    stub_shippo_status(&shippo, PRE_TRANSIT_TRACKING, "PRE_TRANSIT").await;
    stub_shippo_status(&shippo, RETURNED_TRACKING, "RETURNED").await;
    stub_shippo_status(&shippo, UNKNOWN_TRACKING, "WEIRD_STATUS").await;
    stub_shippo_error(&shippo, ERROR_TRACKING).await;

    let client =
        OracleClient::new(&oracle.base_url).with_expected_public_key_hex(TEST_PUBLIC_KEY_HEX)?;

    let cases = vec![
        success_case(
            &client,
            "delivered",
            DELIVERED_TRACKING,
            OracleStatus::Delivered,
        )
        .await,
        success_case(
            &client,
            "transit",
            TRANSIT_TRACKING,
            OracleStatus::InTransit,
        )
        .await,
        success_case(
            &client,
            "pre_transit",
            PRE_TRANSIT_TRACKING,
            OracleStatus::PreTransit,
        )
        .await,
        success_case(
            &client,
            "not_delivered",
            RETURNED_TRACKING,
            OracleStatus::NotDelivered,
        )
        .await,
        success_case(&client, "unknown", UNKNOWN_TRACKING, OracleStatus::Unknown).await,
        error_case(&client, "upstream_error", ERROR_TRACKING).await,
    ];

    let passed = cases.iter().filter(|case| case.passed).count();
    let failed = cases.len() - passed;
    let report = Report {
        generated_at: chrono::Utc::now().to_rfc3339(),
        sdk_version: env!("CARGO_PKG_VERSION").to_string(),
        passed,
        failed,
        cases,
    };

    let report_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("reports");
    fs::create_dir_all(&report_dir)?;
    fs::write(
        report_dir.join("sdk-integration.json"),
        serde_json::to_string_pretty(&report)?,
    )?;
    fs::write(
        report_dir.join("sdk-integration.md"),
        render_markdown(&report),
    )?;

    assert_eq!(report.failed, 0, "sdk report must be fully green");

    Ok(())
}
