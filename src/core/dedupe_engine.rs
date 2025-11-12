use crate::storage::{
    bloom_filter::BloomFilter, memory_cache::MemoryCache, rocksdb_store::RocksDBStore,
};
// use anyhow::Result;
use crate::api::metrics::Metrics;
use dashmap::DashSet;
use nostr_sdk::{Event, EventId};
use std::sync::Arc;
use tracing::{debug, trace};

/// Multi-layer deduplication engine
/// Layer 1: Bloom filter (fast, in-memory, may have false positives)
/// Layer 2: LRU cache (recent events, exact match)
/// Layer 3: RocksDB (persistent storage, exact match)
/// Layer 4: Concurrent hash set (hot path for very recent events)
pub struct DeduplicationEngine {
    bloom: Arc<BloomFilter>,
    lru_cache: Arc<MemoryCache>,
    rocksdb: Arc<RocksDBStore>,
    hot_set: Arc<DashSet<String>>,
    metrics: Option<Arc<Metrics>>,
}

impl DeduplicationEngine {
    /// Create a new deduplication engine
    pub fn new(rocksdb: Arc<RocksDBStore>) -> Self {
        Self {
            bloom: Arc::new(BloomFilter::new()),
            lru_cache: Arc::new(MemoryCache::new()),
            rocksdb,
            hot_set: Arc::new(DashSet::new()),
            metrics: None,
        }
    }

    /// Create a new deduplication engine with custom capacities
    pub fn new_with_params(
        rocksdb: Arc<RocksDBStore>,
        hot_set_size: usize,
        bloom_capacity: usize,
        lru_size: usize,
    ) -> Self {
        Self {
            bloom: Arc::new(BloomFilter::with_capacity(bloom_capacity, 0.01)),
            lru_cache: Arc::new(MemoryCache::with_capacity(lru_size)),
            rocksdb,
            hot_set: Arc::new(DashSet::with_capacity(hot_set_size)),
            metrics: None,
        }
    }

    pub fn with_metrics(mut self, metrics: Arc<Metrics>) -> Self {
        self.metrics = Some(metrics);
        self
    }

    /// Warm in-memory structures from RocksDB successful-forward index.
    /// Loads up to `limit` most recent successfully forwarded events into bloom, hot_set and LRU.
    pub async fn warm_from_db(&self, limit: usize) {
        if limit == 0 {
            return;
        }
        let ids = self.rocksdb.load_recent_success_ids(limit).await;
        for id in &ids {
            match EventId::from_hex(&id) {
                Ok(event_id) => {
                    // Best-effort: insert into bloom, lru and hot_set
                    self.bloom.insert(event_id.as_bytes()).await;
                }
                Err(err) => {
                    tracing::warn!("Failed to parse event id {} from RocksDB: {}", id, err);
                    // continue best-effort using the string forms for caches
                }
            }
            self.lru_cache.put(id.clone()).await;
            self.hot_set.insert(id.to_string());
        }
        tracing::info!(
            "Deduplication engine warmed with {} IDs from RocksDB",
            ids.len()
        );
    }

    /// Check if an event is a duplicate
    /// Returns true if duplicate, false if new event
    pub async fn is_duplicate(&self, event: &Event) -> bool {
        let event_id_hex = event.id.to_hex();

        // Layer 0: Hot set check (fastest, for very recent events)
        if self.hot_set.contains(&event_id_hex) {
            trace!("Event {} found in hot set (duplicate)", event_id_hex);
            return true;
        }

        // Layer 1: Bloom filter check (fast, in-memory, may have false positives)
        if self.bloom.contains(event.id.as_bytes()).await {
            // Bloom filter says it might exist, need to verify
            trace!("Event {} might exist (bloom filter positive)", event_id_hex);
        } else {
            // Bloom filter says it doesn't exist, definitely new
            self.bloom.insert(event.id.as_bytes()).await;
            self.hot_set.insert(event_id_hex.clone());
            debug!("New event {} added to bloom filter", event_id_hex);
            return false;
        }

        // Layer 2: LRU cache check (recent events, exact match)
        if self.lru_cache.contains(&event_id_hex).await {
            trace!("Event {} found in LRU cache (duplicate)", event_id_hex);
            self.hot_set.insert(event_id_hex);
            if let Some(m) = &self.metrics {
                m.duplicates_filtered.inc();
            }
            return true;
        }

        // Layer 3: RocksDB check (persistent storage, exact match)
        if self.rocksdb.exists(&event_id_hex).await {
            // Found in persistent storage, add to cache layers
            self.lru_cache.put(event_id_hex.clone()).await;
            self.hot_set.insert(event_id_hex.clone());
            trace!("Event {} found in RocksDB (duplicate)", event_id_hex);
            if let Some(m) = &self.metrics {
                m.duplicates_filtered.inc();
            }
            return true;
        }

        // New event - store in all layers
        debug!("New event {} detected, storing in all layers", event_id_hex);

        // Store in persistent storage
        if let Err(e) = self.rocksdb.store_event(event).await {
            tracing::error!("Failed to store event {} in RocksDB: {}", event_id_hex, e);
        }

        // Store in cache layers
        self.lru_cache.put(event_id_hex.clone()).await;
        self.hot_set.insert(event_id_hex);

        // Limit hot set size to prevent unbounded growth
        let hot_set_capacity = self.hot_set.capacity();
        if self.hot_set.len() >= hot_set_capacity {
            // Clear half of the hot set (simple eviction strategy)
            let keys: Vec<String> = self
                .hot_set
                .iter()
                .take(hot_set_capacity / 2)
                .map(|s| s.to_string())
                .collect();
            for key in keys {
                self.hot_set.remove(&key);
            }
        }

        false
    }

    /// Get statistics about the deduplication engine
    pub async fn get_stats(&self) -> DedupeStats {
        DedupeStats {
            bloom_filter_size: 0, // Bloom filter doesn't expose size
            lru_cache_size: self.lru_cache.len().await,
            hot_set_size: self.hot_set.len(),
            rocksdb_approximate_count: self.rocksdb.approximate_count().await,
        }
    }
}

/// Statistics about the deduplication engine
#[derive(Debug, Clone)]
pub struct DedupeStats {
    pub bloom_filter_size: usize,
    pub lru_cache_size: usize,
    pub hot_set_size: usize,
    pub rocksdb_approximate_count: u64,
}
