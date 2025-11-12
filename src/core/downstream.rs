use crate::storage::rocksdb_store::RocksDBStore;
use anyhow::{Context, Result};
use flume::Receiver;
use nostr_sdk::Event;
use serde_json;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tracing::{error, info};

/// Downstream forwarder that can send events via TCP or HTTP to multiple endpoints
pub struct DownstreamForwarder {
    tcp_endpoints: Vec<String>,
    rest_endpoints: Vec<String>,
    client: Arc<reqwest::Client>,
    rocksdb: Arc<RocksDBStore>,
}

impl DownstreamForwarder {
    /// Create a new downstream forwarder
    pub fn new(
        tcp_endpoints: Vec<String>,
        rest_endpoints: Vec<String>,
        rocksdb: Arc<RocksDBStore>,
    ) -> Self {
        Self {
            tcp_endpoints,
            rest_endpoints,
            client: Arc::new(reqwest::Client::new()),
            rocksdb,
        }
    }

    /// Forward events from a receiver channel
    pub async fn forward_events(self, rx: Receiver<Event>) -> Result<()> {
        let tcp_endpoints = self.tcp_endpoints.clone();
        let rest_endpoints = self.rest_endpoints.clone();
        let client = self.client.clone();
        let rocksdb = self.rocksdb.clone();

        loop {
            match rx.recv_async().await {
                Ok(event) => {
                    let mut all_ok = true;
                    // Forward to all TCP endpoints in parallel
                    if !tcp_endpoints.is_empty() {
                        let mut tcp_tasks = Vec::new();
                        for endpoint in &tcp_endpoints {
                            let endpoint = endpoint.clone();
                            let event = event.clone();
                            tcp_tasks.push(tokio::spawn(async move {
                                Self::forward_via_tcp(&endpoint, &event).await
                            }));
                        }
                        // Wait for all TCP forwards to complete (fire and forget errors)
                        for task in tcp_tasks {
                            if let Ok(Err(e)) = task.await {
                                error!("Failed to forward event via TCP: {}", e);
                                all_ok = false;
                            }
                        }
                    }

                    // Forward to all REST endpoints in parallel
                    if !rest_endpoints.is_empty() {
                        let mut rest_tasks = Vec::new();
                        for endpoint in &rest_endpoints {
                            let endpoint = endpoint.clone();
                            let event = event.clone();
                            let client = client.clone();
                            rest_tasks.push(tokio::spawn(async move {
                                Self::forward_via_rest(&endpoint, &event, &client).await
                            }));
                        }
                        // Wait for all REST forwards to complete (fire and forget errors)
                        for task in rest_tasks {
                            if let Ok(Err(e)) = task.await {
                                error!("Failed to forward event via REST: {}", e);
                                all_ok = false;
                            }
                        }
                    }

                    if all_ok {
                        if let Err(e) = rocksdb.mark_forward_success(&event.id.to_hex()).await {
                            error!("Failed to mark forward success: {}", e);
                        }
                    }
                }
                Err(_) => {
                    info!("Downstream forwarder: event channel closed");
                    break;
                }
            }
        }

        Ok(())
    }

    /// Forward a single event via TCP
    async fn forward_via_tcp(endpoint: &str, event: &Event) -> Result<()> {
        let mut stream = TcpStream::connect(endpoint)
            .await
            .with_context(|| format!("Failed to connect to TCP endpoint: {}", endpoint))?;

        let serialized = serde_json::to_vec(event).context("Failed to serialize event to JSON")?;

        // Send length prefix (4 bytes) + data
        let len = serialized.len() as u32;
        stream.write_all(&len.to_be_bytes()).await?;
        stream.write_all(&serialized).await?;
        stream.flush().await?;

        Ok(())
    }

    /// Forward a single event via HTTP REST
    async fn forward_via_rest(
        endpoint: &str,
        event: &Event,
        client: &reqwest::Client,
    ) -> Result<()> {
        let response = client
            .post(endpoint)
            .json(event)
            .send()
            .await
            .with_context(|| format!("Failed to send POST request to {}", endpoint))?;

        if !response.status().is_success() {
            anyhow::bail!("REST endpoint returned error status: {}", response.status());
        }

        Ok(())
    }
}
