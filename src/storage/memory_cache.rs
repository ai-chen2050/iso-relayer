use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::Arc;
use tokio::sync::RwLock;

/// LRU cache for recent event IDs
/// Capacity: 100,000 recent events
pub struct MemoryCache {
    cache: Arc<RwLock<LruCache<String, ()>>>,
}

impl MemoryCache {
    /// Create a new LRU cache with custom capacity
    pub fn with_capacity(size: usize) -> Self {
        let capacity = NonZeroUsize::new(size.max(1)).unwrap();
        let cache = LruCache::new(capacity);
        Self {
            cache: Arc::new(RwLock::new(cache)),
        }
    }

    /// Create a new LRU cache with capacity for 100,000 items
    pub fn new() -> Self {
        let capacity = NonZeroUsize::new(100_000).unwrap();
        let cache = LruCache::new(capacity);
        Self {
            cache: Arc::new(RwLock::new(cache)),
        }
    }

    /// Check if an event ID exists in the cache
    pub async fn contains(&self, event_id: &str) -> bool {
        let cache = self.cache.read().await;
        cache.contains(event_id)
    }

    /// Insert an event ID into the cache
    pub async fn put(&self, event_id: String) {
        let mut cache = self.cache.write().await;
        cache.put(event_id, ());
    }

    /// Get the current size of the cache
    pub async fn len(&self) -> usize {
        let cache = self.cache.read().await;
        cache.len()
    }
}

impl Default for MemoryCache {
    fn default() -> Self {
        Self::new()
    }
}
