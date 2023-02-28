use crate::error::{MessageCacheError, SendBufferError, SendManagerError};
use crate::lorawan_protocol::{LoRaWanProtocol, MessageType};
use crate::routing;
use crate::send_buffers::AnnouncementSendBuffer;
use crate::send_buffers::BundleSendBuffer;
use crate::send_buffers::SendBuffer;
use crate::AppState;
use chirpstack_gwb_integration::downlinks::predefined_parameters::DataRate;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, MutexGuard};
use tracing::{error, info, trace, warn};

/// Manages sending of LoRaWAN frames.
pub struct SendManager {
    /// Messages received from a connected gateway to be relayed.
    relay_msgs: Arc<Mutex<Vec<(LoRaWanProtocol, DataRate)>>>,
    /// Max amount of queued relay messaged.
    max_relay_msgs: usize,
    /// Bundles to be sent.
    bundle_buffers: Arc<Mutex<Vec<BundleSendBuffer>>>,
    /// Max amount of queued [`BundleSendBuffer`].
    max_bundle_buffers: usize,
    /// Announcements to be sent.
    announcement_buffers: Arc<Mutex<Vec<AnnouncementSendBuffer>>>,
    /// Max amount of queued [`AnnouncementSendBuffer`].
    max_announcement_buffers: usize,
    /// Delay between sends.
    delay_between_sends: std::time::Duration,
}

impl SendManager {
    /// Create a new [`SendManager`].
    /// Takes the maximum amount of queued entry per queue and the delay between send operations.
    pub fn new(
        max_relay_msgs: usize,
        max_bundle_buffers: usize,
        max_announcement_buffers: usize,
        delay_between_sends: std::time::Duration,
    ) -> Self {
        Self {
            relay_msgs: Arc::new(Mutex::new(Vec::with_capacity(max_relay_msgs))),
            max_relay_msgs,
            bundle_buffers: Arc::new(Mutex::new(Vec::with_capacity(max_bundle_buffers))),
            max_bundle_buffers,
            announcement_buffers: Arc::new(Mutex::new(Vec::with_capacity(
                max_announcement_buffers,
            ))),
            max_announcement_buffers,
            delay_between_sends,
        }
    }

    /// Task to consolidate incoming messages, bundles and announcements into the [`SendManager`]
    /// queues. Needs to be spawned into an async task and kept running.
    pub async fn consolidate_send_items_task(
        &self,
        mut relay_receiver: mpsc::Receiver<(LoRaWanProtocol, DataRate)>,
        mut bundle_send_buffer_receiver: mpsc::Receiver<BundleSendBuffer>,
        mut announcement_send_buffer_receiver: mpsc::Receiver<AnnouncementSendBuffer>,
    ) {
        loop {
            tokio::select! {
                Some(relay_msg) = relay_receiver.recv() => {
                    trace!("Received relay message");
                    let mut lock = self.relay_msgs.lock().await;
                    if lock.len() >= self.max_relay_msgs {
                        warn!("Max amount of queued relay messages reached, dropping message");
                        continue
                    }
                    lock.push(relay_msg);
                },
                Some(bundle_send_buffer) = bundle_send_buffer_receiver.recv() =>  {
                    trace!("Received bundle send buffer");
                    let mut lock = self.bundle_buffers.lock().await;
                    if lock.len() >= self.max_bundle_buffers {
                        warn!("Max amount of queued bundle buffers reached, dropping message");
                        continue
                    }
                    lock.push(bundle_send_buffer);
                },
                Some(announcement_send_buffer) = announcement_send_buffer_receiver.recv() =>  {
                    trace!("Received announcement send buffer");
                    let mut lock = self.announcement_buffers.lock().await;
                    if lock.len() >= self.max_announcement_buffers {
                        warn!("Max amount of queued announcement buffers reached, dropping message");
                        continue
                    }
                    lock.push(announcement_send_buffer);
                },
            }
        }
    }

    /// Task to send at periodic intervals. Needs to be spawned into an async task and kept running.
    pub async fn periodic_send_task(&self, state: Arc<AppState>) {
        // If we encounter an error before we send, we want to be able to skip the delay to not miss
        // a send opportunity.
        let mut skip_delay = false;
        loop {
            if !skip_delay {
                trace!("Starting sleep");
                tokio::time::sleep(self.delay_between_sends).await;
                trace!("Ending sleep");
            } else {
                trace!("Skipping delay");
                skip_delay = false;
            }

            // relay messages
            {
                trace!("Checking for relay message");
                let mut relay_msgs_lock = self.relay_msgs.lock().await;
                if let Some((relay_msg, data_rate)) = relay_msgs_lock.pop() {
                    match relay_msg.convert_to_lorawan_phy_payload() {
                        Ok(phy_payload) => {
                            trace!("Sending payload to flooding");
                            routing::flooding(phy_payload, state.clone(), data_rate).await;
                            continue;
                        }
                        Err(err) => {
                            error!(%err);
                            skip_delay = true;
                            continue;
                        }
                    }
                }
            }

            // Next bundle fragment payload
            {
                trace!("Checking for bundle fragment");
                let bundle_buffer_lock = self.bundle_buffers.lock().await;
                match process_send_buffer_queue(bundle_buffer_lock, state.clone()).await {
                    Ok(()) => {}
                    Err(SendManagerError::NoSendBufferInQueue) => {}
                    Err(_) => {
                        skip_delay = true;
                        continue;
                    }
                }
            }

            // Next announcement fragment payload
            {
                trace!("Checking for announcement fragment");
                let announcement_buffer_lock = self.announcement_buffers.lock().await;
                match process_send_buffer_queue(announcement_buffer_lock, state.clone()).await {
                    Ok(()) => {}
                    Err(SendManagerError::NoSendBufferInQueue) => {}
                    Err(_) => {
                        skip_delay = true;
                        continue;
                    }
                }
            }
        }
    }
}

/// Process a buffer payload. If a payload is available, the payload is sent via [`flooding`].
///
/// # Errors
///
/// Returns an error if:
/// - there is no payload left.
/// - the payload cannot be converted to a LoRaWAN phy payload.
async fn process_send_buffer_payload(
    send_buffer_ref: &mut impl SendBuffer,
    state: Arc<AppState>,
) -> Result<(), SendBufferError> {
    let lorawan_packet = send_buffer_ref.next_payload()?;
    add_bundle_to_message_cache(&lorawan_packet, state.clone())?;
    let phy_payload = lorawan_packet.convert_to_lorawan_phy_payload()?;
    let data_rate = send_buffer_ref.data_rate();
    routing::flooding(phy_payload, state.clone(), data_rate).await;
    Ok(())
}

/// Process a send buffer queue. If a payload is available, the payload is processed by the
/// [`process_send_buffer_payload`] function.
///
/// # Errors
///
/// Returns an error if:
/// - the send buffer does not contain any more fragments.
/// - there is no send buffer in the queue.
/// - the [`process_send_buffer_payload`] function returned an error.
async fn process_send_buffer_queue(
    mut send_buffer_vec: MutexGuard<'_, Vec<impl SendBuffer>>,
    state: Arc<AppState>,
) -> Result<(), SendManagerError> {
    if let Some(entry_ref) = send_buffer_vec.first_mut() {
        if entry_ref.remaining_fragments().is_none() {
            send_buffer_vec.pop();
            let err = SendManagerError::NoRemainingFragments;
            info!(%err);
            Err(err)
        } else if let Err(err) = process_send_buffer_payload(entry_ref, state.clone()).await {
            error!(%err);
            Err(err.into())
        } else {
            Ok(())
        }
    } else {
        let err = SendManagerError::NoSendBufferInQueue;
        info!(%err);
        Err(err)
    }
}

/// If the `lorawan_frame` is part of a Bundle, the packet is added to the message cache.
///
/// # Errors
///
/// Returns an error if the `lorawan_frame` is already present in the message cache.
fn add_bundle_to_message_cache(
    lorawan_frame: &LoRaWanProtocol,
    state: Arc<AppState>,
) -> Result<(), MessageCacheError> {
    match &lorawan_frame.msg_type {
        MessageType::Bundle {
            source,
            timestamp,
            payload,
            ..
        } => state
            .message_cache
            .insert(source, timestamp, payload.fragment.index()),
        MessageType::Announcement { .. } => Ok(()),
    }
}
