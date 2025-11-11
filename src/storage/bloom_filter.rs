use bloom::{ASMS, BloomFilter as BloomFilterLib};
use std::sync::Arc;
use tokio::sync::RwLock;

/// In-memory Bloom filter for fast duplicate detection
/// Capacity: 10 million events with ~1% false positive rate
pub struct BloomFilter {
    filter: Arc<RwLock<BloomFilterLib>>,
}

impl BloomFilter {
    /// Create a new Bloom filter with custom capacity and false positive rate
    pub fn with_capacity(capacity: usize, false_positive_rate: f64) -> Self {
        let filter = BloomFilterLib::with_rate(false_positive_rate as f32, capacity as u32);
        Self {
            filter: Arc::new(RwLock::new(filter)),
        }
    }

    /// Create a new Bloom filter with capacity for 10 million items
    pub fn new() -> Self {
        // Create bloom filter with 10M capacity and 1% false positive rate
        let filter = BloomFilterLib::with_rate(0.01, 10_000_000);
        Self {
            filter: Arc::new(RwLock::new(filter)),
        }
    }

    /// Check if an event ID might exist (fast check, may have false positives)
    pub async fn contains(&self, event_id: &[u8; 32]) -> bool {
        let filter = self.filter.read().await;
        filter.contains(event_id)
    }

    /// Insert an event ID into the bloom filter
    pub async fn insert(&self, event_id: &[u8; 32]) {
        let mut filter = self.filter.write().await;
        filter.insert(event_id);
    }

    /// Clear the bloom filter (useful for testing or reset)
    pub async fn clear(&self) {
        let mut filter = self.filter.write().await;
        *filter = BloomFilterLib::with_rate(0.01, 10_000_000);
    }
}

impl Default for BloomFilter {
    fn default() -> Self {
        Self::new()
    }
}
