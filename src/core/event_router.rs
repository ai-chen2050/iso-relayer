use anyhow::Result;
use flume::{Receiver, Sender};
use nostr_sdk::Event;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, info};

use crate::api::metrics::Metrics;
use crate::core::dedupe_engine::DeduplicationEngine;

/// Wrapper for Event to enable sorting by timestamp
#[derive(Clone)]
struct EventWrapper {
    event: Event,
    timestamp: u64,
}

impl PartialEq for EventWrapper {
    fn eq(&self, other: &Self) -> bool {
        self.timestamp == other.timestamp
    }
}

impl Eq for EventWrapper {}

impl PartialOrd for EventWrapper {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for EventWrapper {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.timestamp.cmp(&other.timestamp)
    }
}

/// Event router that sorts events by timestamp and routes to downstream systems
pub struct EventRouter {
    dedupe_engine: Arc<DeduplicationEngine>,
    batch_size: usize,
    max_latency: Duration,
    downstream_tx: Sender<Event>,
    pending_events: Arc<RwLock<Vec<EventWrapper>>>,
    metrics: Option<Arc<Metrics>>,
}

impl EventRouter {
    /// Create a new event router
    pub fn new(
        dedupe_engine: Arc<DeduplicationEngine>,
        batch_size: usize,
        max_latency: Duration,
        downstream_tx: Sender<Event>,
    ) -> Self {
        Self {
            dedupe_engine,
            batch_size,
            max_latency,
            downstream_tx,
            pending_events: Arc::new(RwLock::new(Vec::new())),
            metrics: None,
        }
    }

    /// Attach metrics collection
    pub fn with_metrics(mut self, metrics: Arc<Metrics>) -> Self {
        self.metrics = Some(metrics);
        self
    }

    /// Process incoming event stream, deduplicate, and route to downstream
    pub async fn process_stream(self, input: Receiver<Event>) -> Result<()> {
        let mut last_flush = Instant::now();

        loop {
            // Use timeout to periodically flush even if no new events arrive
            let timeout = tokio::time::sleep(self.max_latency);
            tokio::pin!(timeout);

            tokio::select! {
                // Receive new event
                result = input.recv_async() => {
                    match result {
                        Ok(event) => {
                            // Deduplication check
                            if !self.dedupe_engine.is_duplicate(&event).await {
                                // Add to pending events (will be sorted before flushing)
                                let timestamp = event.created_at.as_secs();
                                let wrapper = EventWrapper {
                                    event,
                                    timestamp,
                                };

                                let mut pending = self.pending_events.write().await;
                                pending.push(wrapper);
                                if let Some(m) = &self.metrics {
                                    m.events_in_queue.set(pending.len() as f64);
                                }

                                // If we have enough events, flush a batch
                                if pending.len() >= self.batch_size {
                                    drop(pending);
                                    self.flush_batch().await?;
                                    last_flush = Instant::now();
                                }
                            }
                        }
                        Err(_) => {
                            info!("Event stream closed, flushing remaining events");
                            self.flush_all().await?;
                            break;
                        }
                    }
                }
                // Timeout - flush if we have events and enough time has passed
                _ = timeout => {
                    let pending = self.pending_events.read().await;
                    if !pending.is_empty() && last_flush.elapsed() >= self.max_latency {
                        drop(pending);
                        let start = Instant::now();
                        self.flush_batch().await?;
                        if let Some(m) = &self.metrics {
                            let elapsed = start.elapsed().as_secs_f64();
                            m.processing_latency.observe(elapsed);
                        }
                        last_flush = Instant::now();
                    }
                }
            }
        }

        Ok(())
    }

    /// Flush a batch of events sorted by timestamp
    async fn flush_batch(&self) -> Result<()> {
        let mut pending = self.pending_events.write().await;
        let batch_size = self.batch_size.min(pending.len());

        if batch_size == 0 {
            return Ok(());
        }

        // Sort by timestamp (ascending - oldest first)
        pending.sort();

        // Take the oldest events (first batch_size events)
        let batch: Vec<Event> = pending
            .drain(0..batch_size)
            .map(|wrapper| wrapper.event)
            .collect();

        drop(pending);

        // Send events to downstream in timestamp order
        for event in batch {
            if let Err(e) = self.downstream_tx.send_async(event).await {
                error!("Failed to send event to downstream: {}", e);
            }
            if let Some(m) = &self.metrics {
                m.events_processed.inc();
            }
        }

        debug!("Flushed batch of {} events", batch_size);
        if let Some(m) = &self.metrics {
            let remaining = self.pending_events.read().await.len();
            m.events_in_queue.set(remaining as f64);
        }
        Ok(())
    }

    /// Flush all remaining events
    async fn flush_all(&self) -> Result<()> {
        let mut pending = self.pending_events.write().await;
        let count = pending.len();

        // Sort by timestamp before flushing
        pending.sort();

        let events: Vec<Event> = pending.drain(..).map(|wrapper| wrapper.event).collect();

        for event in events {
            if let Err(e) = self.downstream_tx.send_async(event).await {
                error!("Failed to send event to downstream: {}", e);
            }
            if let Some(m) = &self.metrics {
                m.events_processed.inc();
            }
        }

        info!("Flushed all remaining {} events", count);
        if let Some(m) = &self.metrics {
            m.events_in_queue.set(0.0);
        }
        Ok(())
    }
}
