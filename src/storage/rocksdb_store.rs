use anyhow::{Context, Result};
use nostr_sdk::Event;
use rocksdb::{Options, DB};
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
        
        let db = DB::open(&opts, path)
            .context("Failed to open RocksDB database")?;
        
        Ok(Self {
            db: Arc::new(RwLock::new(db)),
        })
    }

    /// Check if an event ID exists in the database
    pub async fn exists(&self, event_id: &str) -> bool {
        let db = self.db.read().await;
        db.get(event_id.as_bytes()).is_ok()
    }

    /// Store an event in the database
    pub async fn store_event(&self, event: &Event) -> Result<()> {
        let event_id = event.id.to_string();
        let serialized = serde_json::to_vec(event)
            .context("Failed to serialize event")?;
        
        let db = self.db.write().await;
        db.put(event_id.as_bytes(), serialized)
            .context("Failed to store event in RocksDB")?;
        
        Ok(())
    }

    /// Retrieve an event by ID
    pub async fn get_event(&self, event_id: &str) -> Result<Option<Event>> {
        let db = self.db.read().await;
        match db.get(event_id.as_bytes()) {
            Ok(Some(data)) => {
                let event: Event = serde_json::from_slice(&data)
                    .context("Failed to deserialize event")?;
                Ok(Some(event))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(anyhow::anyhow!("Database error: {}", e)),
        }
    }

    /// Delete an event by ID
    pub async fn delete_event(&self, event_id: &str) -> Result<()> {
        let db = self.db.write().await;
        db.delete(event_id.as_bytes())
            .context("Failed to delete event from RocksDB")?;
        Ok(())
    }

    /// Get approximate number of events in the database
    pub async fn approximate_count(&self) -> u64 {
        let db = self.db.read().await;
        // This is an approximation, actual count may vary
        db.iterator(rocksdb::IteratorMode::Start).count() as u64
    }
}

