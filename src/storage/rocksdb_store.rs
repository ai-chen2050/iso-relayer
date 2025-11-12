use anyhow::{Context, Result};
use nostr_sdk::Event;
use rocksdb::{DB, IteratorMode, Options};
use serde_json;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Persistent storage using RocksDB for event deduplication and archival
pub struct RocksDBStore {
    db: Arc<RwLock<DB>>,
}

impl RocksDBStore {
    /// Open or create a RocksDB database at the specified path
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);

        // Optimize for write-heavy workload
        opts.set_write_buffer_size(64 * 1024 * 1024); // 64MB
        opts.set_max_write_buffer_number(3);
        opts.set_min_write_buffer_number_to_merge(1);

        // Enable compression
        opts.set_compression_type(rocksdb::DBCompressionType::Lz4);

        let db = DB::open(&opts, path).context("Failed to open RocksDB database")?;

        Ok(Self {
            db: Arc::new(RwLock::new(db)),
        })
    }

    #[inline]
    fn key_event(event_id: &str) -> Vec<u8> {
        // Event payload storage
        let mut key = Vec::with_capacity(4 + event_id.len());
        key.extend_from_slice(b"evt:");
        key.extend_from_slice(event_id.as_bytes());
        key
    }

    #[inline]
    fn key_forward_status(event_id: &str) -> Vec<u8> {
        // Forwarding status for quick lookup
        let mut key = Vec::with_capacity(4 + event_id.len());
        key.extend_from_slice(b"fwd:");
        key.extend_from_slice(event_id.as_bytes());
        key
    }

    #[inline]
    fn key_success_index(epoch_ms: i64, event_id: &str) -> Vec<u8> {
        // Time-ordered index for recent successful deliveries
        // Format: "succ:{016x}:{event_id}" where time is hex, zero-padded for lexical sort
        // Using hex keeps keys ASCII and sorted lexicographically in time order.
        let mut key = Vec::with_capacity(5 + 16 + 1 + event_id.len());
        key.extend_from_slice(b"succ:");
        let ts_hex = format!("{:016x}", epoch_ms as u64);
        key.extend_from_slice(ts_hex.as_bytes());
        key.push(b':');
        key.extend_from_slice(event_id.as_bytes());
        key
    }

    /// Check if an event ID exists in the database
    pub async fn exists(&self, event_id: &str) -> bool {
        let db = self.db.read().await;
        match db.get(Self::key_event(event_id)) {
            Ok(Some(_)) => true,
            _ => false,
        }
    }

    /// Store an event in the database
    pub async fn store_event(&self, event: &Event) -> Result<()> {
        let event_id = event.id.to_string();
        let serialized = serde_json::to_vec(event).context("Failed to serialize event")?;

        let db = self.db.write().await;
        db.put(Self::key_event(&event_id), serialized)
            .context("Failed to store event in RocksDB")?;

        Ok(())
    }

    /// Retrieve an event by ID
    pub async fn get_event(&self, event_id: &str) -> Result<Option<Event>> {
        let db = self.db.read().await;
        match db.get(Self::key_event(event_id)) {
            Ok(Some(data)) => {
                let event: Event =
                    serde_json::from_slice(&data).context("Failed to deserialize event")?;
                Ok(Some(event))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(anyhow::anyhow!("Database error: {}", e)),
        }
    }

    /// Delete an event by ID
    pub async fn delete_event(&self, event_id: &str) -> Result<()> {
        let db = self.db.write().await;
        db.delete(Self::key_event(event_id))
            .context("Failed to delete event from RocksDB")?;
        Ok(())
    }

    /// Get approximate number of events in the database
    pub async fn approximate_count(&self) -> u64 {
        let db = self.db.read().await;
        // This is an approximation, actual count may vary
        db.iterator(rocksdb::IteratorMode::Start).count() as u64
    }

    /// Mark an event as successfully forwarded to downstream(s)
    pub async fn mark_forward_success(&self, event_id: &str) -> Result<()> {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let mut batch = rocksdb::WriteBatch::default();
        batch.put(Self::key_forward_status(event_id), b"1");
        batch.put(Self::key_success_index(now_ms, event_id), &[]);
        let db = self.db.write().await;
        db.write(batch).context("Failed to mark forward success")?;
        Ok(())
    }

    /// Check whether an event has been marked as successfully forwarded
    pub async fn is_forward_success(&self, event_id: &str) -> bool {
        let db = self.db.read().await;
        matches!(db.get(Self::key_forward_status(event_id)), Ok(Some(_)))
    }

    /// Load up to `limit` most recent successfully forwarded event IDs (most recent first)
    pub async fn load_recent_success_ids(&self, limit: usize) -> Vec<String> {
        if limit == 0 {
            return Vec::new();
        }
        let db = self.db.read().await;
        let mut iter = db.iterator(IteratorMode::End);
        let mut result = Vec::with_capacity(limit.min(1024));
        while result.len() < limit {
            match iter.next() {
                Some(Ok((k, _v))) => {
                    // Only consider keys with "succ:" prefix
                    if k.starts_with(b"succ:") {
                        // key format: succ:{016x}:{event_id}
                        if let Some(pos) = k.iter().position(|b| *b == b':') {
                            // find the second colon
                            let second = k
                                .iter()
                                .enumerate()
                                .skip(pos + 1)
                                .find(|(_, b)| **b == b':');
                            if let Some((second_idx, _)) = second {
                                // event id starts after second colon
                                let event_id_bytes = &k[second_idx + 1..];
                                if let Ok(event_id) = std::str::from_utf8(event_id_bytes) {
                                    result.push(event_id.to_string());
                                }
                            }
                        }
                    }
                }
                _ => break,
            }
        }
        result
    }
}
