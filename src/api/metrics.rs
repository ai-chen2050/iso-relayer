use prometheus::{register_gauge, register_histogram, register_int_counter, Gauge, Histogram, IntCounter};

/// Metrics for monitoring the relay system
pub struct Metrics {
    pub events_processed: IntCounter,
    pub duplicates_filtered: IntCounter,
    pub processing_latency: Histogram,
    pub memory_usage: Gauge,
    pub active_connections: Gauge,
    pub events_in_queue: Gauge,
}

impl Metrics {
    /// Create and register all metrics
    pub fn new() -> Result<Self, prometheus::Error> {
        Ok(Self {
            events_processed: register_int_counter!(
                "events_processed_total",
                "Total events processed"
            )?,
            duplicates_filtered: register_int_counter!(
                "duplicates_filtered_total",
                "Total duplicates filtered"
            )?,
            processing_latency: register_histogram!(
                "processing_latency_seconds",
                "Event processing latency in seconds"
            )?,
            memory_usage: register_gauge!(
                "memory_usage_mb",
                "Memory usage in Million Bytes"
            )?,
            active_connections: register_gauge!(
                "active_connections",
                "Number of active relay connections"
            )?,
            events_in_queue: register_gauge!(
                "events_in_queue",
                "Number of events waiting in queue"
            )?,
        })
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new().expect("Failed to create metrics")
    }
}
