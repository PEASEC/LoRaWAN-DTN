//! Processing of incoming uplinks.

use crate::graceful_shutdown::ShutdownAgent;
use crate::lora_modulation_extraction::extract_modulation_info_from_uplink_tx_info;
use crate::lorawan_protocol::{parse_phy_payload, LoRaWanPacket};
use crate::receive_buffers::ReceiveBufferManager;
use crate::AppState;
use async_trait::async_trait;
use chirpstack_gwb_integration::downlinks::predefined_parameters::DataRate;
use chirpstack_gwb_integration::runtime::callbacks::EventUpCallback;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TrySendError;
use tracing::{error, instrument, trace};

/// Uplink callback sending incoming uplink frames to the uplink processing task.
#[derive(Debug)]
pub struct UplinkCallback {
    /// Channel to send the gateway ID and the uplink frame.
    pub uplink_callback_tx: mpsc::Sender<(String, chirpstack_api::gw::UplinkFrame)>,
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
        if let Err(err) = self.uplink_callback_tx.try_send((gateway_id, up_event)) {
            error!(%err);
        }
    }
}

/// Task to processes incoming uplinks.
///
/// Checks whether the uplink was already seen within the timeout window. If not, adds it to the
/// uplink cache, checks the addressing to determine whether it was addressed to this instance or
/// should be routed further.
#[instrument(skip_all)]
pub async fn uplink_processor_task(
    mut uplink_rx: mpsc::Receiver<(String, chirpstack_api::gw::UplinkFrame)>,
    relay_tx: mpsc::Sender<(Box<dyn LoRaWanPacket>, DataRate)>,
    state: Arc<AppState>,
    mut shutdown_agent: ShutdownAgent,
) {
    trace!("Starting up");
    let mut receive_buffer_manager = ReceiveBufferManager::new(state.clone());
    loop {
        let uplink = tokio::select! {
            uplink = uplink_rx.recv() => { uplink}
            _ = shutdown_agent.await_shutdown() => {
                trace!("Shutting down");
                return
            }
        };

        if let Some((gateway_id, uplink)) = uplink {
            trace!(
                "Received uplink from gateway \"{gateway_id}\": {:?}",
                uplink.phy_payload
            );

            match parse_phy_payload(&uplink.phy_payload) {
                Ok(parsed_packet) => {
                    if state
                        .packet_cache
                        .insert(&uplink.phy_payload)
                        .await
                        .is_err()
                    {
                        trace!("Uplink already seen");
                        continue;
                    }

                    let end_device_id_match = {
                        if let Some(destination) = parsed_packet.packet_destination() {
                            let end_device_ids_lock = state.end_device_ids.lock().await;
                            !end_device_ids_lock.contains(&destination.into())
                        } else {
                            false
                        }
                    };
                    if end_device_id_match {
                        trace!("Uplink end device ID did not match, relaying");

                        let modulation_info =
                            match extract_modulation_info_from_uplink_tx_info(uplink.tx_info) {
                                Ok(modulation_info) => modulation_info,
                                Err(err) => {
                                    error!(%err);
                                    continue;
                                }
                            };
                        let data_rate = match DataRate::from_raw_bandwidth_and_spreading_factor(
                            modulation_info.bandwidth,
                            modulation_info.spreading_factor,
                        ) {
                            Ok(data_rate) => data_rate,
                            Err(err) => {
                                error!(%err);
                                continue;
                            }
                        };

                        // relay packet
                        if let Err(err) = relay_tx.try_send((parsed_packet, data_rate)) {
                            match err {
                                TrySendError::Full(_) => {
                                    error!("Relay channel is full, dropping relay packet");
                                }
                                TrySendError::Closed(_) => {
                                    error!("Relay channel is closed");
                                }
                            }
                        }
                        continue;
                    }
                    receive_buffer_manager.process_packet(parsed_packet);
                    continue;
                }
                Err(e) => {
                    error!("The following is caused by a parsing error or the incoming payload not being proprietary");
                    error!(%e);
                }
            }
        }
    }
}
