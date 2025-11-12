use axum::{
    Router,
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{delete, get, post},
};
use prometheus::{Encoder, TextEncoder};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use crate::api::metrics::Metrics;
use crate::core::relay_pool::RelayPool;
use crate::core::dedupe_engine::DeduplicationEngine;

#[derive(Clone)]
pub struct AppState {
    pub pool: Arc<RelayPool>,
    pub dedupe: Arc<DeduplicationEngine>,
    pub metrics: Arc<Metrics>,
}

/// Create the REST API router
pub fn create_router(pool: Arc<RelayPool>, dedupe: Arc<DeduplicationEngine>, metrics: Arc<Metrics>) -> Router {
    let state = AppState { pool, dedupe, metrics };
    Router::new()
        .route("/health", get(health))
        .route("/metrics", get(prometheus_metrics))
        .route("/status", get(status))
        .route("/api/metrics/summary", get(metrics_summary))
        .route("/api/metrics/memory", get(memory))
        .route("/api/relays", get(list_relays))
        .route("/api/relays/add", post(add_relay))
        .route("/api/relays/remove", delete(remove_relay))
        .with_state(state)
}

/// Health check endpoint
async fn health() -> Json<serde_json::Value> {
    Json(json!({
        "status": "healthy",
        "service": "iso-relayer"
    }))
}

/// Metrics endpoint for Prometheus
async fn prometheus_metrics() -> Result<String, StatusCode> {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();

    encoder
        .encode(&metric_families, &mut buffer)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(String::from_utf8_lossy(&buffer).to_string())
}

/// Get connection status
async fn status(State(state): State<AppState>) -> Json<serde_json::Value> {
    let statuses = state.pool.get_connection_statuses().await;
    let active = state.pool.active_connections();
    let deque_status = state.dedupe.get_stats().await;

    Json(json!({
        "active_connections": active,
        "connections": statuses.iter().map(|(url, status)| {
            json!({
                "url": url,
                "status": format!("{:?}", status)
            })
        }).collect::<Vec<_>>(),
        "deduplication_engine": {
            "bloom_filter_size": deque_status.bloom_filter_size,
            "lru_cache_size": deque_status.lru_cache_size,
            "rocksdb_entry_count": deque_status.rocksdb_approximate_count,
            "hot_set_size": deque_status.hot_set_size,
        }
    }))
}

/// Request body for adding a relay
#[derive(Debug, Deserialize)]
struct AddRelayRequest {
    url: String,
}

/// Request body for removing a relay
#[derive(Debug, Deserialize)]
struct RemoveRelayRequest {
    url: String,
}

/// Response for relay operations
#[derive(Debug, Serialize)]
struct RelayResponse {
    success: bool,
    message: String,
}

/// Add a new relay
async fn add_relay(
    State(state): State<AppState>,
    Json(payload): Json<AddRelayRequest>,
) -> Result<Json<RelayResponse>, StatusCode> {
    match state.pool.connect_and_subscribe(payload.url.clone()).await {
        Ok(_) => Ok(Json(RelayResponse {
            success: true,
            message: format!("Successfully connected to relay: {}", payload.url),
        })),
        Err(e) => {
            tracing::error!("Failed to add relay {}: {}", payload.url, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Remove a relay
async fn remove_relay(
    State(state): State<AppState>,
    Json(payload): Json<RemoveRelayRequest>,
) -> Result<Json<RelayResponse>, StatusCode> {
    match state.pool.disconnect_relay(&payload.url).await {
        Ok(_) => Ok(Json(RelayResponse {
            success: true,
            message: format!("Successfully disconnected relay: {}", payload.url),
        })),
        Err(e) => {
            tracing::error!("Failed to remove relay {}: {}", payload.url, e);
            Err(StatusCode::NOT_FOUND)
        }
    }
}

/// List all relays
async fn list_relays(State(state): State<AppState>) -> Json<serde_json::Value> {
    let _relay_urls = state.pool.list_relays();
    let statuses = state.pool.get_connection_statuses().await;

    let mut relay_info = Vec::new();
    for (url, status) in statuses {
        relay_info.push(json!({
            "url": url,
            "status": format!("{:?}", status)
        }));
    }

    Json(json!({
        "relays": relay_info,
        "count": relay_info.len()
    }))
}

/// Summary metrics endpoint (JSON)
async fn metrics_summary(State(state): State<AppState>) -> Json<serde_json::Value> {
    let m = &state.metrics;
    // Convert the kb to MB（1 MB = 1024 * 1024 bytes）
    let memory_usage_mb = m.memory_usage.get() as f64 / 1024.0;
    Json(serde_json::json!({
        "events_processed_total": m.events_processed.get(),
        "duplicates_filtered_total": m.duplicates_filtered.get(),
        "events_in_queue": m.events_in_queue.get(),
        "active_connections": m.active_connections.get(),
        "memory_usage_mb": memory_usage_mb,
    }))
}

/// Memory-only endpoint
async fn memory(State(state): State<AppState>) -> Json<serde_json::Value> {
    // Convert the byte to MB
    let memory_usage_mb = state.metrics.memory_usage.get() as f64 / 1024.0;
    Json(serde_json::json!({
        "memory_usage_mb": memory_usage_mb,
    }))
}
