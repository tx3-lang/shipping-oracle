//! Generates `backend/reports/integration.{json,md}` exercising every status
//! mapping plus the upstream-error path. Run as part of CI; the artifacts are
//! used as milestone evidence (E2). Each case is captured rather than
//! asserted, so a single failure still produces a full report.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use ed25519_dalek::{Signature, Verifier};
use pallas::codec::minicbor;
use serde::Serialize;
use serde_json::json;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use shipping_oracle::api;
use shipping_oracle::config::Config;
use shipping_oracle::models::SignedOracleResponse;
use shipping_oracle::oracle_service::{
    OracleService, blake2b256, load_signing_key, plutus_oracle_data,
};
use shipping_oracle::shipment::ShipmentClient;

const TEST_SK_HEX: &str = "0101010101010101010101010101010101010101010101010101010101010101";
const SHIPPO_CARRIER: &str = "shippo";
const FROZEN_TIMESTAMP: i64 = 1712000000;

#[derive(Serialize, Default, Clone)]
struct CaseReport {
    name: String,
    description: String,
    carrier: String,
    tracking_number: String,
    expected_status: String,
    actual_status: Option<String>,
    timestamp: Option<i64>,
    carrier_hash: Option<String>,
    tracking_number_hash: Option<String>,
    cbor_hex: Option<String>,
    public_key: Option<String>,
    signature: Option<String>,
    signature_verified: bool,
    cbor_matches_plaintext: bool,
    http_status: u16,
    passed: bool,
    errors: Vec<String>,
}

#[derive(Serialize)]
struct Report {
    generated_at: String,
    backend_version: String,
    cases: Vec<CaseReport>,
    passed: usize,
    failed: usize,
}

struct TestServer {
    base_url: String,
    _handle: JoinHandle<()>,
}

async fn start_oracle(shippo_base_url: String) -> TestServer {
    let config = Config {
        listen_address: "127.0.0.1:0".to_string(),
        shippo_api_key: "test-token".to_string(),
        oracle_sk: TEST_SK_HEX.to_string(),
        oracle_pkh: "00".repeat(28),
        oracle_address: "addr_test1placeholder".to_string(),
        trp_url: "http://localhost:8000".to_string(),
        trp_api_key: None,
    };
    let shipment_client =
        ShipmentClient::with_base_url(&config, shippo_base_url).expect("shipment client");
    let signing_key = load_signing_key(&config.oracle_sk).expect("load sk");
    let service = Arc::new(OracleService::with_clock(
        shipment_client,
        signing_key,
        Box::new(|| FROZEN_TIMESTAMP),
    ));

    let listener = TcpListener::bind(&config.listen_address)
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("local addr");
    let app = api::create_router(service);
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.expect("oracle server");
    });

    TestServer {
        base_url: format!("http://{addr}"),
        _handle: handle,
    }
}

async fn stub_shippo_status(server: &MockServer, tracking_number: &str, status: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/tracks/{SHIPPO_CARRIER}/{tracking_number}")))
        .and(header("Authorization", "ShippoToken test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "carrier": SHIPPO_CARRIER,
            "tracking_number": tracking_number,
            "tracking_status": {
                "status": status,
                "status_details": "stub"
            }
        })))
        .mount(server)
        .await;
}

fn check_signature(response: &SignedOracleResponse) -> Result<()> {
    let vk_bytes = hex::decode(&response.public_key).context("vk hex")?;
    let vk: [u8; 32] = vk_bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("vk wrong length"))?;
    let vk = ed25519_dalek::VerifyingKey::from_bytes(&vk).context("vk decode")?;
    let sig_bytes = hex::decode(&response.signature).context("sig hex")?;
    let sig_bytes: [u8; 64] = sig_bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("sig wrong length"))?;
    let signature = Signature::from_bytes(&sig_bytes);
    let cbor_bytes = hex::decode(&response.cbor_hex).context("cbor hex")?;
    vk.verify(&cbor_bytes, &signature)
        .context("ed25519 verify")?;
    Ok(())
}

fn check_cbor_matches_plaintext(response: &SignedOracleResponse) -> Result<()> {
    let carrier_hash = blake2b256(response.plaintext.carrier.as_bytes());
    let tracking_hash = blake2b256(response.plaintext.tracking_number.as_bytes());
    if hex::encode(&carrier_hash) != response.data.carrier_hash {
        bail!("carrier_hash differs from blake2b(plaintext.carrier)");
    }
    if hex::encode(&tracking_hash) != response.data.tracking_number_hash {
        bail!("tracking_number_hash differs from blake2b(plaintext.tracking_number)");
    }
    let plutus = plutus_oracle_data(
        carrier_hash,
        tracking_hash,
        response.data.status.as_bytes().to_vec(),
        response.data.timestamp,
    );
    let expected = hex::encode(minicbor::to_vec(&plutus).context("re-encode plutus")?);
    if expected != response.cbor_hex {
        bail!("cbor_hex does not round-trip from declared fields");
    }
    Ok(())
}

async fn run_success_case(
    base_url: &str,
    name: &str,
    description: &str,
    tracking_number: &str,
    expected_status: &str,
) -> CaseReport {
    let mut case = CaseReport {
        name: name.to_string(),
        description: description.to_string(),
        carrier: SHIPPO_CARRIER.to_string(),
        tracking_number: tracking_number.to_string(),
        expected_status: expected_status.to_string(),
        ..Default::default()
    };

    let response = match reqwest::get(format!(
        "{base_url}/v1/shipment?carrier={SHIPPO_CARRIER}&tracking_number={tracking_number}",
    ))
    .await
    {
        Ok(r) => r,
        Err(e) => {
            case.errors.push(format!("http error: {e}"));
            return case;
        }
    };
    case.http_status = response.status().as_u16();
    if !response.status().is_success() {
        case.errors
            .push(format!("non-success http status {}", case.http_status));
        return case;
    }
    let body: SignedOracleResponse = match response.json().await {
        Ok(b) => b,
        Err(e) => {
            case.errors.push(format!("decode body: {e}"));
            return case;
        }
    };

    case.actual_status = Some(body.data.status.clone());
    case.timestamp = Some(body.data.timestamp);
    case.carrier_hash = Some(body.data.carrier_hash.clone());
    case.tracking_number_hash = Some(body.data.tracking_number_hash.clone());
    case.cbor_hex = Some(body.cbor_hex.clone());
    case.public_key = Some(body.public_key.clone());
    case.signature = Some(body.signature.clone());

    match check_signature(&body) {
        Ok(_) => case.signature_verified = true,
        Err(e) => case.errors.push(format!("signature verify: {e:#}")),
    }
    match check_cbor_matches_plaintext(&body) {
        Ok(_) => case.cbor_matches_plaintext = true,
        Err(e) => case.errors.push(format!("cbor align: {e:#}")),
    }

    if let Some(actual) = &case.actual_status {
        if actual != expected_status {
            case.errors.push(format!(
                "status mismatch: expected {expected_status}, got {actual}"
            ));
        }
    }

    case.passed = case.errors.is_empty();
    case
}

async fn run_error_case(
    base_url: &str,
    name: &str,
    description: &str,
    tracking_number: &str,
    expected_http: u16,
) -> CaseReport {
    let mut case = CaseReport {
        name: name.to_string(),
        description: description.to_string(),
        carrier: SHIPPO_CARRIER.to_string(),
        tracking_number: tracking_number.to_string(),
        expected_status: format!("http {expected_http}"),
        ..Default::default()
    };

    let response = match reqwest::get(format!(
        "{base_url}/v1/shipment?carrier={SHIPPO_CARRIER}&tracking_number={tracking_number}",
    ))
    .await
    {
        Ok(r) => r,
        Err(e) => {
            case.errors.push(format!("http error: {e}"));
            return case;
        }
    };
    case.http_status = response.status().as_u16();
    if case.http_status == expected_http {
        case.passed = true;
    } else {
        case.errors.push(format!(
            "expected http {expected_http}, got {}",
            case.http_status
        ));
    }
    case
}

fn render_markdown(report: &Report) -> String {
    let mut out = String::new();
    out.push_str("# Shipping Oracle — Integration Report\n\n");
    out.push_str(&format!("- **Generated:** `{}`\n", report.generated_at));
    out.push_str(&format!(
        "- **Backend version:** `{}`\n",
        report.backend_version
    ));
    out.push_str(&format!(
        "- **Result:** {} passed / {} failed (out of {})\n\n",
        report.passed,
        report.failed,
        report.cases.len()
    ));

    out.push_str("## Cases\n\n");
    out.push_str(
        "| # | Case | Carrier | Tracking | Expected | Actual | HTTP | Sig | CBOR | Result |\n",
    );
    out.push_str("|---|---|---|---|---|---|---|---|---|---|\n");
    for (i, c) in report.cases.iter().enumerate() {
        out.push_str(&format!(
            "| {} | {} | `{}` | `{}` | `{}` | `{}` | {} | {} | {} | {} |\n",
            i + 1,
            c.name,
            c.carrier,
            c.tracking_number,
            c.expected_status,
            c.actual_status.as_deref().unwrap_or("—"),
            c.http_status,
            if c.signature_verified { "✅" } else { "—" },
            if c.cbor_matches_plaintext {
                "✅"
            } else {
                "—"
            },
            if c.passed { "✅ PASS" } else { "❌ FAIL" },
        ));
    }

    out.push_str("\n## Descriptions\n\n");
    for c in &report.cases {
        out.push_str(&format!("- **{}** — {}\n", c.name, c.description));
    }

    out.push_str("\n## Errors\n\n");
    let mut had_errors = false;
    for c in &report.cases {
        if !c.errors.is_empty() {
            had_errors = true;
            out.push_str(&format!("- **{}**\n", c.name));
            for e in &c.errors {
                out.push_str(&format!("  - {e}\n"));
            }
        }
    }
    if !had_errors {
        out.push_str("(none)\n");
    }

    out
}

fn write_reports(report: &Report) -> Result<()> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dir = manifest_dir.join("reports");
    fs::create_dir_all(&dir).context("create reports dir")?;
    let json_path = dir.join("integration.json");
    fs::write(&json_path, serde_json::to_string_pretty(&report)?)
        .with_context(|| format!("write {}", json_path.display()))?;
    let md_path = dir.join("integration.md");
    fs::write(&md_path, render_markdown(report))
        .with_context(|| format!("write {}", md_path.display()))?;
    Ok(())
}

#[tokio::test]
async fn integration_report() -> Result<()> {
    let shippo = MockServer::start().await;
    stub_shippo_status(&shippo, "SHIPPO_DELIVERED", "DELIVERED").await;
    stub_shippo_status(&shippo, "SHIPPO_TRANSIT", "TRANSIT").await;
    stub_shippo_status(&shippo, "SHIPPO_PRE_TRANSIT", "PRE_TRANSIT").await;
    stub_shippo_status(&shippo, "SHIPPO_FAILURE", "FAILURE").await;
    stub_shippo_status(&shippo, "SHIPPO_RETURNED", "RETURNED").await;
    stub_shippo_status(&shippo, "SHIPPO_UNKNOWN", "WEIRD_STATUS").await;
    Mock::given(method("GET"))
        .and(path(format!("/tracks/{SHIPPO_CARRIER}/SHIPPO_BOOM")))
        .respond_with(ResponseTemplate::new(500).set_body_string("upstream error"))
        .mount(&shippo)
        .await;

    let server = start_oracle(shippo.uri()).await;

    let cases = vec![
        run_success_case(
            &server.base_url,
            "delivered",
            "Final state: package was delivered.",
            "SHIPPO_DELIVERED",
            "DELIVERED",
        )
        .await,
        run_success_case(
            &server.base_url,
            "in_transit",
            "Package currently in transit.",
            "SHIPPO_TRANSIT",
            "IN_TRANSIT",
        )
        .await,
        run_success_case(
            &server.base_url,
            "pre_transit",
            "Carrier created label, not yet picked up.",
            "SHIPPO_PRE_TRANSIT",
            "PRE_TRANSIT",
        )
        .await,
        run_success_case(
            &server.base_url,
            "failure",
            "Delivery failed (mapped to NOT_DELIVERED).",
            "SHIPPO_FAILURE",
            "NOT_DELIVERED",
        )
        .await,
        run_success_case(
            &server.base_url,
            "returned",
            "Package returned to sender (mapped to NOT_DELIVERED).",
            "SHIPPO_RETURNED",
            "NOT_DELIVERED",
        )
        .await,
        run_success_case(
            &server.base_url,
            "unknown",
            "Unrecognised carrier status falls back to UNKNOWN.",
            "SHIPPO_UNKNOWN",
            "UNKNOWN",
        )
        .await,
        run_error_case(
            &server.base_url,
            "upstream_error",
            "Carrier API returns 5xx → oracle surfaces 502.",
            "SHIPPO_BOOM",
            502,
        )
        .await,
    ];

    let passed = cases.iter().filter(|c| c.passed).count();
    let failed = cases.len() - passed;
    let report = Report {
        generated_at: chrono::Utc::now().to_rfc3339(),
        backend_version: env!("CARGO_PKG_VERSION").to_string(),
        cases,
        passed,
        failed,
    };

    write_reports(&report)?;

    if report.failed > 0 {
        bail!("integration report: {} case(s) failed", report.failed);
    }
    Ok(())
}
