//! Send manager responsible for sending packets.

use crate::graceful_shutdown::ShutdownAgent;
use crate::lorawan_protocol::LoRaWanPacket;
use crate::send_buffers::BundleSendBuffer;
use chirpstack_gwb_integration::downlinks::predefined_parameters::DataRate;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::{instrument, trace, warn};

/// Queues of LoRaWAN frames and [`BundleSendBuffer`].
#[derive(Debug)]
pub struct QueueManager {
    /// Packets received from a connected gateway to be relayed.
    pub(crate) relay_packet_queue: Arc<Mutex<Vec<(Box<dyn LoRaWanPacket>, DataRate)>>>,
    /// Max amount of queued relay packets.
    pub(crate) max_relay_packets: usize,
    /// Bundles to be sent.
    pub(crate) bundle_send_buffer_queue: Arc<Mutex<Vec<BundleSendBuffer>>>,
    /// Max amount of queued [`BundleSendBuffer`].
    pub(crate) max_bundle_buffers: usize,
}

impl QueueManager {
    /// Create a new [`QueueManager`].
    /// Takes the maximum amount of queued entries per queue.
    pub fn new(
        relay_packet_queue: Arc<Mutex<Vec<(Box<dyn LoRaWanPacket>, DataRate)>>>,
        max_relay_packets: usize,
        bundle_send_buffer_queue: Arc<Mutex<Vec<BundleSendBuffer>>>,
        max_bundle_buffers: usize,
    ) -> Self {
        Self {
            relay_packet_queue,
            max_relay_packets,
            bundle_send_buffer_queue,
            max_bundle_buffers,
        }
    }

    /// Task to collect incoming packets, bundles into the [`QueueManager`]
    /// queues. Needs to be spawned into an async task and kept running.
    #[instrument(skip_all)]
    pub async fn collect_send_items_task(
        &self,
        mut relay_rx: mpsc::Receiver<(Box<dyn LoRaWanPacket>, DataRate)>,
        mut bundle_send_buffer_rx: mpsc::Receiver<BundleSendBuffer>,
        mut shutdown_agent: ShutdownAgent,
    ) {
        trace!("Starting up");
        loop {
            tokio::select! {
                Some(relay_packet) = relay_rx.recv() => {
                    trace!("Received relay packet");
                    let mut relay_packet_lock = self.relay_packet_queue.lock().await;
                    if relay_packet_lock.len() >= self.max_relay_packets {
                        warn!("Max amount of queued relay packets reached, dropping packet");
                        continue
                    }
                    relay_packet_lock.push(relay_packet);
                },
                Some(bundle_send_buffer) = bundle_send_buffer_rx.recv() =>  {
                    trace!("Received bundle send buffer");
                    let mut bundle_buffers_lock = self.bundle_send_buffer_queue.lock().await;
                    if bundle_buffers_lock.len() >= self.max_bundle_buffers {
                        warn!("Max amount of queued bundle buffers reached, dropping buffer");
                        continue
                    }
                    bundle_buffers_lock.push(bundle_send_buffer);
                },
                _ = shutdown_agent.await_shutdown() => {
                    trace!("Shutting down");
                    return;
                }
            }
        }
    }
}
