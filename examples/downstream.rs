use axum::{Router, extract::Json, http::StatusCode, response::IntoResponse, routing::post};
use serde_json::Value;
use std::net::SocketAddr;
use tracing::info;
use tracing_subscriber::EnvFilter;

async fn handle_event(Json(payload): Json<Value>) -> impl IntoResponse {
    info!("=== Received Nostr Event ===");
    info!(
        "{}",
        serde_json::to_string_pretty(&payload).unwrap()
    );
    info!("============================");

    (StatusCode::OK, "Event received successfully")
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new("debug"))
        .init();

    let app = Router::new().route("/events", post(handle_event));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    info!("Server running on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
