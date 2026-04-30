use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use serde_json::json;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::models::{ShipmentQuery, SignedOracleResponse};
use crate::oracle_service::OracleService;

pub fn create_router(oracle_service: Arc<OracleService>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/shipment", get(shipment))
        .with_state(oracle_service)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}

async fn health() -> impl IntoResponse {
    Json(json!({ "status": "ok" }))
}

async fn shipment(
    State(oracle_service): State<Arc<OracleService>>,
    Query(query): Query<ShipmentQuery>,
) -> Result<Json<SignedOracleResponse>, ApiError> {
    let response = oracle_service
        .attest(&query.carrier, &query.tracking_number)
        .await
        .map_err(ApiError::Upstream)?;
    Ok(Json(response))
}

pub enum ApiError {
    Upstream(anyhow::Error),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            ApiError::Upstream(err) => {
                tracing::error!(error = ?err, "oracle attestation failed");
                (StatusCode::BAD_GATEWAY, err.to_string())
            }
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}
