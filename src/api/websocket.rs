use axum::{
    Router,
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::Response,
    routing::get,
};
use flume::Receiver;
use futures_util::{SinkExt, StreamExt};
use nostr_sdk::Event;
use serde_json;
use std::sync::Arc;
use tracing::{error, info};

// use crate::core::relay_pool::RelayPool;

/// WebSocket handler for streaming events to downstream systems
async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(event_rx): State<Arc<Receiver<Event>>>,
) -> Response {
    ws.on_upgrade(|socket| handle_socket(socket, event_rx))
}

/// Handle individual WebSocket connection
async fn handle_socket(socket: WebSocket, event_rx: Arc<Receiver<Event>>) {
    info!("New WebSocket connection established");

    let (mut sender, mut receiver) = socket.split();

    // Spawn task to send events to client
    let send_task = tokio::spawn(async move {
        let event_rx = event_rx.clone();
        while let Ok(event) = event_rx.recv_async().await {
            let json = match serde_json::to_string(&event) {
                Ok(j) => j,
                Err(e) => {
                    error!("Failed to serialize event: {}", e);
                    continue;
                }
            };

            if let Err(e) = sender.send(Message::Text(json.into())).await {
                error!("Failed to send WebSocket message: {}", e);
                break;
            }
        }
    });

    // Spawn task to receive messages from client (for ping/pong, etc.)
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Close(_) => {
                    info!("WebSocket connection closed by client");
                    break;
                }
                Message::Ping(_data) => {
                    // Handle ping (pong will be sent automatically by axum)
                }
                _ => {}
            }
        }
    });

    // Wait for either task to complete
    tokio::select! {
        _ = send_task => {}
        _ = recv_task => {}
    }

    info!("WebSocket connection closed");
}

/// Create WebSocket router
pub fn create_websocket_router(event_rx: Arc<Receiver<Event>>) -> Router {
    Router::new()
        .route("/ws", get(websocket_handler))
        .with_state(event_rx)
}
