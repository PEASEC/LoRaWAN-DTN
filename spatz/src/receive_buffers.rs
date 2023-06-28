//! Receive buffers collecting incoming fragments.
//! Receive buffer manager to manage all receive buffers.

mod bundle;
mod hop2hop;

use crate::end_device_id::EndDeviceId;
use crate::lorawan_protocol::{
    BundleFragmentOffsetHash, Hop2HopFragment, LoRaWanPacket, LocalAnnouncement,
};
use crate::AppState;
pub use bundle::BundleReceiveBuffer;
use chrono::{DateTime, Utc};
pub use hop2hop::Hop2HopReceiveBuffer;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{error, trace};

/// Convert a unix timestamp to a [`bp7::DtnTime`].
pub fn unix_ts_to_dtn_time(timestamp: u64) -> bp7::DtnTime {
    (timestamp - bp7::dtntime::SECONDS1970_TO2K) * 1000
}

/// Manages receive buffers.
pub struct ReceiveBufferManager {
    /// Application state.
    state: Arc<AppState>,
    /// Bundle receive buffer.
    bundle_receive_buffers: HashMap<
        (
            EndDeviceId,
            EndDeviceId,
            DateTime<Utc>,
            Option<BundleFragmentOffsetHash>,
        ),
        BundleReceiveBuffer,
    >,
    /// Hop2Hop receive buffer.
    hop2hop_receive_buffers: HashMap<u32, Hop2HopReceiveBuffer>,
}

impl ReceiveBufferManager {
    /// Create a new [`ReceiveBufferManager`].
    pub fn new(state: Arc<AppState>) -> Self {
        Self {
            state,
            bundle_receive_buffers: HashMap::new(),
            hop2hop_receive_buffers: HashMap::new(),
        }
    }

    /// Process a packet into the corresponding buffer or create a new buffer if there is no
    /// corresponding buffer.
    pub fn process_packet(&mut self, mut packet: Box<dyn LoRaWanPacket>) {
        if let Some(bundle_fragment) = packet.as_bundle_packet_mut() {
            match self.bundle_receive_buffers.entry((
                bundle_fragment.destination(),
                bundle_fragment.source(),
                bundle_fragment.timestamp(),
                bundle_fragment.bundle_fragment_offset_hash(),
            )) {
                Entry::Occupied(mut entry) => {
                    if let Err(err) = entry.get_mut().process_packet(bundle_fragment) {
                        error!(%err);
                        return;
                    }
                    if entry.get().is_combinable() {
                        trace!("Bundle is combinable");
                        let receive_buffer = entry.remove();
                        match receive_buffer.combine() {
                            Ok(bp7_bundle) => self.send_pb7_bundle_to_ws(bp7_bundle),
                            Err(err) => {
                                error!(%err);
                            }
                        }
                    }
                }
                Entry::Vacant(entry) => {
                    let receive_buffer = BundleReceiveBuffer::from(bundle_fragment);

                    if receive_buffer.is_combinable() {
                        trace!("Bundle is combinable");
                        match receive_buffer.combine() {
                            Ok(bp7_bundle) => self.send_pb7_bundle_to_ws(bp7_bundle),
                            Err(err) => {
                                error!(%err);
                            }
                        }
                    } else {
                        entry.insert(receive_buffer);
                    }
                }
            }
        } else if let Some(hop_2_hop_fragment) =
            packet.as_any_mut().downcast_mut::<Hop2HopFragment>()
        {
            match self
                .hop2hop_receive_buffers
                .entry(hop_2_hop_fragment.packet_hash())
            {
                Entry::Occupied(mut entry) => {
                    if let Err(err) = entry.get_mut().process_packet(hop_2_hop_fragment) {
                        error!(%err);
                        return;
                    }
                    if entry.get().is_combinable() {
                        trace!("Hop2Hop packet is combinable");
                        let receive_buffer = entry.remove();
                        match receive_buffer.combine() {
                            Ok(combined_packet) => self.process_packet(combined_packet),
                            Err(err) => {
                                error!(%err);
                            }
                        }
                    }
                }
                Entry::Vacant(entry) => {
                    let receive_buffer = match Hop2HopReceiveBuffer::try_from(hop_2_hop_fragment) {
                        Ok(receive_buffer) => receive_buffer,
                        Err(err) => {
                            error!(%err);
                            return;
                        }
                    };

                    if receive_buffer.is_combinable() {
                        trace!("Hop2Hop packet is combinable");
                        match receive_buffer.combine() {
                            Ok(combined_packet) => self.process_packet(combined_packet),
                            Err(err) => {
                                error!(%err);
                            }
                        }
                    } else {
                        entry.insert(receive_buffer);
                    }
                }
            }
        } else if let Some(local_announcement) =
            packet.as_any_mut().downcast_mut::<LocalAnnouncement>()
        {
            let location = if let Some(location) = local_announcement.location() {
                format!("{:?}", location.as_float_coords())
            } else {
                "no location".to_owned()
            };
            trace!(
                "Received local announcement with location: \"{}\" and end device IDs: {:?}",
                location,
                local_announcement.end_device_ids_ref()
            );
            // TODO add to local_announcement management
        }
    }

    /// Send [`bp7::Bundle`] to all connected websocket clients.
    /// If no clients are connected, the bundle is dropped.
    fn send_pb7_bundle_to_ws(&self, bundle: bp7::Bundle) {
        if self.state.bundles_to_ws.receiver_count() > 0 {
            if let Err(e) = self.state.bundles_to_ws.send(bundle) {
                error!(%e);
            }
        } else {
            error!("No WS client connected, bundle dropped");
        }
    }
}
