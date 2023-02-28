use crate::end_device_id::EndDeviceId;
use crate::lora_modulation_extraction::extract_modulation_info_from_uplink_tx_info;
use crate::lorawan_protocol::parse_phy_payload;
use crate::lorawan_protocol::{Fragment, LoRaWanProtocol, MessageType};
use crate::receive_buffers::AnnouncementReceiveBuffer;
use crate::receive_buffers::{BundleReceiveBuffer, CombinedBundle};
use crate::AppState;
use async_trait::async_trait;
use chirpstack_gwb_integration::downlinks::predefined_parameters::DataRate;
use chirpstack_gwb_integration::runtime::callbacks::EventUpCallback;
use chrono::{DateTime, Utc};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc::error::TrySendError;
use tracing::{error, instrument, trace};

/// Uplink callback sending incoming uplink frames to the uplink processing task.
#[derive(Debug)]
pub struct UplinkCallback {
    pub sender: tokio::sync::mpsc::Sender<(String, chirpstack_api::gw::UplinkFrame)>,
}

#[async_trait]
impl EventUpCallback for UplinkCallback {
    /// Send incoming uplinks via the channel in the [`UplinkCallback`] struct.
    async fn dispatch_up_event(
        &self,
        gateway_id: String,
        up_event: chirpstack_api::gw::UplinkFrame,
    ) {
        trace!("Dispatch up event called");
        if let Err(err) = self.sender.try_send((gateway_id, up_event)) {
            error!(%err);
        }
    }
}

/// Task to processes incoming uplinks.
///
/// Checks whether the uplink was already seen within the timeout window. If not, adds it to the
/// uplink cache, checks the addressing to determine whether it was addressed to this instance or
/// should be routed further.
#[instrument(skip(uplink_receiver, relay_sender, state))]
pub async fn uplink_processor_task(
    mut uplink_receiver: tokio::sync::mpsc::Receiver<(String, chirpstack_api::gw::UplinkFrame)>,
    relay_sender: tokio::sync::mpsc::Sender<(LoRaWanProtocol, DataRate)>,
    state: Arc<AppState>,
) {
    trace!("Started uplink processor");
    let bundles_to_ws_sender = state.bundles_to_ws.clone();
    let mut bundle_receive_buffer_map: HashMap<(EndDeviceId, DateTime<Utc>), BundleReceiveBuffer> =
        HashMap::new();
    let _announcement_receive_buffer_map: HashMap<EndDeviceId, AnnouncementReceiveBuffer> =
        HashMap::new();
    while let Some((gateway_id, uplink)) = uplink_receiver.recv().await {
        trace!(
            "Received uplink from gateway \"{gateway_id}\": {:?}",
            uplink.phy_payload
        );
        let (phy_payload, tx_info) = (uplink.phy_payload, uplink.tx_info);

        match parse_phy_payload(phy_payload) {
            Ok(lorawan_protocol_packet) => match &lorawan_protocol_packet.msg_type {
                MessageType::Bundle {
                    destination,
                    source,
                    timestamp,
                    payload,
                } => {
                    trace!("Timestamp of incoming uplink: {}", timestamp.timestamp());
                    if state
                        .message_cache
                        .insert(source, timestamp, payload.fragment.index())
                        .is_err()
                    {
                        trace!("Uplink already seen");
                        continue;
                    }

                    let end_device_id_found = {
                        let end_device_ids_lock =
                            state.end_device_ids.lock().expect("Lock poisoned");
                        end_device_ids_lock.contains(&destination.into())
                    };

                    if !end_device_id_found {
                        trace!("Uplink end device ID did not match.");

                        let Ok(modulation_info) = extract_modulation_info_from_uplink_tx_info(tx_info) else {
                            continue;
                        };
                        let Ok(data_rate) = DataRate::from_raw_bandwidth_and_spreading_factor(modulation_info.bandwidth, modulation_info.spreading_factor) else {
                            continue;
                        };
                        // relay packet
                        if let Err(err) =
                            relay_sender.try_send((lorawan_protocol_packet, data_rate))
                        {
                            match err {
                                TrySendError::Full(_) => {
                                    error!("Relay channel is full, dropping relay frame");
                                }
                                TrySendError::Closed(_) => {
                                    error!("Relay channel is closed");
                                }
                            }
                        }

                        continue;
                    }
                    trace!("Uplink end device ID match.");

                    if let Fragment::Yes { total_amount, .. } = &payload.fragment {
                        let bundle_receive_buffer_ref =
                            match bundle_receive_buffer_map.entry((*source, *timestamp)) {
                                Entry::Occupied(entry) => entry.into_mut(),
                                Entry::Vacant(entry) => {
                                    let receive_buffer = BundleReceiveBuffer::new(
                                        *destination,
                                        *source,
                                        *timestamp,
                                        *total_amount,
                                    );
                                    entry.insert(receive_buffer)
                                }
                            };

                        if let Err(e) = bundle_receive_buffer_ref.process_payload(payload.clone()) {
                            error!(%e);
                            continue;
                        }

                        if bundle_receive_buffer_ref.missing_fragments().is_none() {
                            let Some(bundle_receive_buffer) = bundle_receive_buffer_map
                                .remove(&(*source, *timestamp))
                                else {
                                error!("Failed to retrieve bundle receive buffer from map even though it should be present");
                                continue;
                            };

                            match bundle_receive_buffer.combine() {
                                Ok(combined_bundle) => {
                                    send_combined_bundle_via_channel(
                                        bundles_to_ws_sender.clone(),
                                        combined_bundle,
                                    );
                                }
                                Err(e) => {
                                    error!(%e);
                                    continue;
                                }
                            }
                        }
                    } else {
                        let combined_bundle = CombinedBundle {
                            destination: *destination,
                            source: *source,
                            timestamp: *timestamp,
                            payload: payload.payload.clone(),
                        };
                        send_combined_bundle_via_channel(
                            bundles_to_ws_sender.clone(),
                            combined_bundle,
                        );
                    }
                }
                MessageType::Announcement { .. } => {
                    todo!()
                }
            },
            Err(e) => {
                error!("The following is caused by a parsing error or the incoming payload not being proprietary");
                error!(%e)
            }
        }
    }
    trace!("Leaving uplink processor task.");
}

/// Convert a [`CombinedBundle`] to a [`bp7::Bundle`] and send it to all connected websocket clients.
/// If no clients are connected, the bundle is dropped.
fn send_combined_bundle_via_channel(
    bundles_to_ws_sender: tokio::sync::broadcast::Sender<bp7::Bundle>,
    combined_bundle: CombinedBundle,
) {
    if bundles_to_ws_sender.receiver_count() > 0 {
        match TryInto::<bp7::Bundle>::try_into(combined_bundle) {
            Ok(bundle) => {
                if let Err(e) = bundles_to_ws_sender.send(bundle) {
                    error!(%e);
                }
            }
            Err(e) => {
                error!(%e);
            }
        }
    } else {
        error!("No WS client connected, bundle dropped");
    }
}
