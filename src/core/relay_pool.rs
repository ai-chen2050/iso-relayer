use crate::api::metrics::Metrics;
use anyhow::{Context, Result};
use dashmap::DashMap;
use flume::{Receiver, Sender};
use nostr_sdk::{Client, Event, Filter, Keys, RelayPoolNotification};
use std::sync::Arc;
use std::sync::Arc as StdArc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// Connection status for a relay
#[derive(Debug, Clone, PartialEq)]
pub enum RelayStatus {
    Connected,
    Disconnected,
    Connecting,
    Error(String),
}

/// Connection state for a single relay
#[derive(Clone)]
pub struct RelayConnection {
    url: String,
    client: Arc<Client>,
    status: Arc<RwLock<RelayStatus>>,
    event_tx: Sender<Event>,
}

/// Pool of relay connections with health checking and load balancing
pub struct RelayPool {
    connections: Arc<DashMap<String, RelayConnection>>,
    health_check_interval: Duration,
    max_connections: usize,
    event_tx: Sender<Event>,
    metrics: Option<StdArc<Metrics>>,
}

impl RelayPool {
    /// Create a new relay pool
    pub fn new(health_check_interval: Duration, max_connections: usize) -> (Self, Receiver<Event>) {
        let (tx, rx) = flume::unbounded();
        let pool = Self {
            connections: Arc::new(DashMap::new()),
            health_check_interval,
            max_connections,
            event_tx: tx,
            metrics: None,
        };
        (pool, rx)
    }

    pub fn with_metrics(mut self, metrics: StdArc<Metrics>) -> Self {
        self.metrics = Some(metrics);
        self
    }

    /// Connect to a relay and subscribe to events
    pub async fn connect_and_subscribe(&self, relay_url: String) -> Result<()> {
        if self.connections.len() >= self.max_connections {
            warn!("Max connections reached, skipping {}", relay_url);
            return Ok(());
        }

        if self.connections.contains_key(&relay_url) {
            info!("Relay {} already connected", relay_url);
            return Ok(());
        }

        info!("Connecting to relay: {}", relay_url);

        let client = Client::new(Keys::generate());

        // Add relay to client
        client
            .add_relay(&relay_url)
            .await
            .context(format!("Failed to add relay: {}", relay_url))?;

        // Connect to relay
        // Connect to relay (no error returned in this version)
        client.connect().await;

        let status = Arc::new(RwLock::new(RelayStatus::Connected));
        let event_tx = self.event_tx.clone();

        // Subscribe to all events (empty filter means all events)
        let filter = Filter::new();
        client
            .subscribe(filter, None)
            .await
            .context("Failed to subscribe to relay")?;

        let connection = RelayConnection {
            url: relay_url.clone(),
            client: Arc::new(client),
            status: status.clone(),
            event_tx: event_tx.clone(),
        };

        self.connections
            .insert(relay_url.clone(), connection.clone());

        // Spawn task to handle events from this relay
        tokio::spawn(Self::handle_relay_events(connection, event_tx));

        info!(
            "Successfully connected and subscribed to relay: {}",
            relay_url
        );
        Ok(())
    }

    /// Handle events from a single relay connection
    async fn handle_relay_events(connection: RelayConnection, event_tx: Sender<Event>) {
        let mut notifications = connection.client.notifications();

        while let Ok(notification) = notifications.recv().await {
            match notification {
                RelayPoolNotification::Event { event, .. } => {
                    if let Err(e) = event_tx.send_async(*event).await {
                        error!("Failed to send event to pipeline: {}", e);
                        break;
                    }
                }
                RelayPoolNotification::Message { message, .. } => {
                    // Handle other message types if needed
                    info!("Received message from {}: {:?}", connection.url, message);
                }
                _ => {}
            }
        }

        warn!("Event stream ended for relay: {}", connection.url);
        *connection.status.write().await = RelayStatus::Disconnected;
    }

    /// Connect to multiple relays in parallel
    pub async fn subscribe_all(&self, relay_urls: Vec<String>) -> Result<()> {
        let tasks: Vec<_> = relay_urls
            .into_iter()
            .map(|url| {
                let pool = self.clone();
                tokio::spawn(async move {
                    if let Err(e) = pool.connect_and_subscribe(url.clone()).await {
                        error!("Failed to connect to {}: {}", url, e);
                    }
                })
            })
            .collect();

        futures::future::join_all(tasks).await;
        Ok(())
    }

    /// Start health checking for all connections
    pub async fn start_health_checks(&self) {
        let connections = self.connections.clone();
        let interval = self.health_check_interval;

        let metrics = self.metrics.clone();
        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);
            loop {
                interval_timer.tick().await;

                for entry in connections.iter() {
                    let connection = entry.value();
                    let current_status = connection.status.read().await.clone();

                    // Check if connection is still alive by trying to get status
                    // In a real implementation, you might want to ping the relay
                    if current_status == RelayStatus::Disconnected {
                        warn!(
                            "Relay {} is disconnected, attempting reconnect",
                            connection.url
                        );
                        // Attempt reconnection
                        connection.client.connect().await;
                        *connection.status.write().await = RelayStatus::Connected;
                    }
                }
                if let Some(m) = &metrics {
                    m.active_connections.set(connections.len() as f64);
                }
            }
        });
    }

    /// Get the number of active connections
    pub fn active_connections(&self) -> usize {
        self.connections.len()
    }

    /// Get connection status for all relays
    pub async fn get_connection_statuses(&self) -> Vec<(String, RelayStatus)> {
        let mut statuses = Vec::new();
        for entry in self.connections.iter() {
            let status = entry.value().status.read().await.clone();
            statuses.push((entry.key().clone(), status));
        }
        statuses
    }

    /// Disconnect and remove a relay
    pub async fn disconnect_relay(&self, relay_url: &str) -> Result<()> {
        if let Some((_, connection)) = self.connections.remove(relay_url) {
            *connection.status.write().await = RelayStatus::Disconnected;
            // Note: The client will be dropped when the connection is removed
            // The handle_relay_events task will naturally terminate
            info!("Disconnected and removed relay: {}", relay_url);
            Ok(())
        } else {
            anyhow::bail!("Relay {} not found", relay_url)
        }
    }

    /// Get list of all relay URLs
    pub fn list_relays(&self) -> Vec<String> {
        self.connections
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
    }
}

impl Clone for RelayPool {
    fn clone(&self) -> Self {
        Self {
            connections: self.connections.clone(),
            health_check_interval: self.health_check_interval,
            max_connections: self.max_connections,
            event_tx: self.event_tx.clone(),
            metrics: self.metrics.clone(),
        }
    }
}
