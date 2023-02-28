use crate::end_device_id::EndDeviceId;
use crate::error::MessageCacheError;
use crate::{AppState, Duration};
use chrono::{DateTime, Utc};
use sha3::Digest;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::trace;

/// Caches hashes of incoming uplinks.
///
/// This is used to check if uplinks where already seen within the timeout period to prevent
/// processing and routing of the same uplink until the timeout has run out.
#[derive(Debug)]
pub struct MessageCache {
    /// HashMap containing the uplink hash and a timestamp.
    cache: Arc<Mutex<HashMap<[u8; 32], DateTime<Utc>>>>,
    /// Timeout duration. Withing this duration, the same uplink will be ignored.
    timeout: Duration,
    /// Interval at which the expired entries are removed from the cache.
    cleanup_interval_seconds: u64,
    /// Reset the timeout if the message is seen again.
    reset_timeout: bool,
}

impl MessageCache {
    /// Create a new [`MessageCache`].
    pub fn new(timeout_minutes: u32, cleanup_interval_seconds: u64, reset_timeout: bool) -> Self {
        MessageCache {
            cache: Arc::new(Mutex::new(HashMap::new())),
            timeout: Duration::minutes(i64::from(timeout_minutes)),
            cleanup_interval_seconds,
            reset_timeout,
        }
    }
    /// Remove all entries of the cache for which the timout has elapsed.
    pub fn remove_expired_messages(&self) {
        trace!("Removing expired messages from message cache");
        let timeout = self.timeout;
        let now = Utc::now();
        self.cache
            .lock()
            .expect("Lock is poisoned")
            .retain(|_hash, timestamp| now - *timestamp < timeout);
    }

    /// Insert a new entry into the cache.
    ///
    /// Depending on the `reset_timeout` field of the [`MessageCache`] struct, the timeout is reset when the
    /// same entry is inserted while already present.
    ///
    /// # Error:
    /// If the entry is already present in the cache, an error is returned.
    pub fn insert(
        &self,
        source: &EndDeviceId,
        timestamp: &DateTime<Utc>,
        fragment_id: Option<u8>,
    ) -> Result<(), MessageCacheError> {
        let fragment_id = fragment_id.unwrap_or(u8::MAX);
        let packet_ident = format!("{}{}{}", source.0, timestamp.timestamp(), fragment_id);
        let packet_hash: [u8; 32] = <[u8; 32]>::from(sha3::Sha3_256::digest(packet_ident));

        let mut cache_lock = self.cache.lock().expect("Lock is poisoned");
        match cache_lock.entry(packet_hash) {
            Entry::Occupied(mut entry) => {
                if Utc::now() - *entry.get() < self.timeout {
                    trace!("Message has already been seen within the timeout duration, skipping");
                    if self.reset_timeout {
                        trace!("Resetting message timeout.");
                        entry.insert(Utc::now());
                    }
                    Err(MessageCacheError::NotTimedOut)
                } else {
                    trace!("Message has already been seen but timeout elapsed, adding to message cache");
                    entry.insert(Utc::now());
                    Ok(())
                }
            }
            Entry::Vacant(entry) => {
                trace!("Message has not been seen before, adding to message cache");
                entry.insert(Utc::now());
                Ok(())
            }
        }
    }
}

/// Task to execute [`MessageCache::remove_expired_messages()`] on the specified interval.
pub async fn cache_clean_task(state: Arc<AppState>) {
    loop {
        state.message_cache.remove_expired_messages();
        tokio::time::sleep(tokio::time::Duration::from_secs(
            state.message_cache.cleanup_interval_seconds,
        ))
        .await;
    }
}

#[allow(clippy::unwrap_used)]
#[cfg(test)]
mod tests {
    use crate::end_device_id::EndDeviceId;
    use crate::MessageCache;
    use chrono::Utc;

    #[test]
    fn message_cache_insert() {
        let message_cache = MessageCache::new(30, 30, false);
        let end_device_id = EndDeviceId(0xabcd);
        let timestamp = Utc::now();
        assert!(message_cache
            .insert(&end_device_id, &timestamp, Some(1))
            .is_ok());
        assert!(message_cache
            .insert(&end_device_id, &timestamp, Some(1))
            .is_err());
    }
}
