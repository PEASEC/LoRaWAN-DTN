//! Packet cache to prevent sending packets that were already sent.

use crate::error::PacketCacheError;
use crate::graceful_shutdown::ShutdownAgent;
use crate::{AppState, Duration};
use chrono::{DateTime, Utc};
use sha3::Digest;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{instrument, trace};

/// Caches hashes of sent and received packets.
///
/// This is used to check if packets where already seen within the timeout period to prevent
/// processing and routing of the same packet until the timeout has run out.
#[derive(Debug)]
pub struct PacketCache {
    /// HashMap containing the uplink hash and a timestamp.
    cache: Arc<Mutex<HashMap<String, DateTime<Utc>>>>,
    /// Timeout duration. Withing this duration, the same uplink will be ignored.
    timeout: Duration,
    /// Interval at which the expired entries are removed from the cache.
    cleanup_interval_seconds: u64,
    /// Reset the timeout if the packet is seen again.
    reset_timeout: bool,
}

impl PacketCache {
    /// Create a new [`PacketCache`].
    pub fn new(
        cache: HashMap<String, DateTime<Utc>>,
        timeout_minutes: u32,
        cleanup_interval_seconds: u64,
        reset_timeout: bool,
    ) -> Self {
        PacketCache {
            cache: Arc::new(Mutex::new(cache)),
            timeout: Duration::minutes(i64::from(timeout_minutes)),
            cleanup_interval_seconds,
            reset_timeout,
        }
    }
    /// Remove all entries of the cache for which the timout has elapsed.
    pub async fn remove_expired_packets(&self) {
        trace!("Removing expired packets from packet cache");
        let timeout = self.timeout;
        let now = Utc::now();
        self.cache
            .lock()
            .await
            .retain(|_hash, timestamp| now - *timestamp < timeout);
    }

    /// Insert a new entry into the cache.
    ///
    /// Depending on the `reset_timeout` field of the [`PacketCache`] struct, the timeout is reset when the
    /// same entry is inserted while already present.
    ///
    /// # Error:
    /// If the entry is already present in the cache, an error is returned.
    pub async fn insert(&self, packet: &[u8]) -> Result<(), PacketCacheError> {
        let packet_hash: [u8; 32] = <[u8; 32]>::from(sha3::Sha3_256::digest(packet));
        // Use the string representation as that can be de-/serialized.
        let packet_hash_string = hex::encode(packet_hash);

        let mut cache_lock = self.cache.lock().await;
        match cache_lock.entry(packet_hash_string) {
            Entry::Occupied(mut entry) => {
                if Utc::now() - *entry.get() < self.timeout {
                    trace!("Packet has already been seen within the timeout duration, skipping");
                    if self.reset_timeout {
                        trace!("Resetting packet timeout.");
                        entry.insert(Utc::now());
                    }
                    Err(PacketCacheError::NotTimedOut)
                } else {
                    trace!(
                        "Packet has already been seen but timeout elapsed, adding to packet cache"
                    );
                    entry.insert(Utc::now());
                    Ok(())
                }
            }
            Entry::Vacant(entry) => {
                trace!("Packet has not been seen before, adding to packet cache");
                entry.insert(Utc::now());
                Ok(())
            }
        }
    }

    /// Returns the contents of the packet cache.
    pub async fn contents(&self) -> HashMap<String, DateTime<Utc>> {
        self.cache.lock().await.clone()
    }
}

/// Task to execute [`PacketCache::remove_expired_packets()`] on the specified interval.
#[instrument(skip_all)]
pub async fn cache_clean_task(state: Arc<AppState>, mut shutdown_agent: ShutdownAgent) {
    trace!("Starting up");
    loop {
        state.packet_cache.remove_expired_packets().await;

        tokio::select! {
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(state.packet_cache.cleanup_interval_seconds)) => {},
            _ = shutdown_agent.await_shutdown() => {
                trace!("Shutting down");
                    return
            }
        };
    }
}

#[allow(clippy::unwrap_used)]
#[cfg(test)]
mod tests {
    use crate::PacketCache;
    use std::collections::HashMap;

    #[tokio::test]
    async fn packet_cache_insert() {
        let packet_cache = PacketCache::new(HashMap::new(), 30, 30, false);
        let packet = [0xFF; 300];
        assert!(packet_cache.insert(&packet).await.is_ok());
        assert!(packet_cache.insert(&packet).await.is_err());
    }
}
