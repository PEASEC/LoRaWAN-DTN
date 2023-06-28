//! Flooding routing algorithm.

use crate::error::NextPacketFromSendBufferError;
use crate::graceful_shutdown::ShutdownAgent;
use crate::routing::{
    create_downlink, create_downlink_item, get_next_payload_from_send_buffer_queue,
    RoutingAlgorithm,
};
use crate::AppState;
use async_trait::async_trait;
use chirpstack_gwb_integration::downlinks::predefined_parameters::{DataRate, Frequency};
use rand::Rng;
use std::sync::Arc;
use tracing::{error, instrument, trace};

/// The flooding routing algorithm.
pub struct Flooding {
    /// The delay betweens send operations.
    delay_between_sends: std::time::Duration,
}

impl Flooding {
    /// Create a new [`Flooding`].
    pub fn new(delay_between_sends: std::time::Duration) -> Self {
        Self {
            delay_between_sends,
        }
    }

    /// Sends the payload from every gateway connected to the ChirpStack.
    #[instrument(skip_all)]
    async fn flooding(
        state: Arc<AppState>,
        payload: Vec<u8>,
        data_rate: DataRate,
        frequency: Frequency,
    ) {
        trace!("Creating downlink item");
        let downlink_item = match create_downlink_item(payload, frequency, data_rate) {
            Ok(downlink_item) => downlink_item,
            Err(err) => {
                error!(%err);
                return;
            }
        };

        trace!("Iterating over gateways");
        for gateway in state.gateway_ids_manager.gateway_ids.lock().await.iter() {
            let downlink = match create_downlink(
                gateway.clone(),
                rand::thread_rng().gen(),
                downlink_item.clone(),
            ) {
                Ok(downlink) => downlink,
                Err(err) => {
                    error!(%err);
                    continue;
                }
            };
            trace!("Enqueuing downlink for gateway: {gateway}");
            if let Err(err) = state.runtime.try_enqueue(gateway, downlink) {
                error!(%err);
            };
        }
    }
}

#[async_trait]
impl RoutingAlgorithm for Flooding {
    async fn routing_task(&self, state: Arc<AppState>, mut shutdown_agent: ShutdownAgent) {
        trace!("Starting up");
        // Hardcoded data rate and frequency
        let data_rate = DataRate::Eu863_870Dr3;
        let frequency = Frequency::Freq868_3;
        // If we encounter an error before we send, we want to be able to skip the delay to not miss
        // a send opportunity.
        let mut skip_delay = false;

        loop {
            if skip_delay {
                trace!("Skipping delay");
                skip_delay = false;
            } else {
                trace!("Starting sleep");
                tokio::select! {
                    _ = tokio::time::sleep(self.delay_between_sends) => {},
                    _ = shutdown_agent.await_shutdown() => {
                        trace!("Shutting down");
                        return
                    }
                };
                trace!("Ending sleep");
            }

            // relay packets
            {
                trace!("Checking for relay packets");

                if let Some((relay_packet, data_rate)) =
                    state.queue_manager.relay_packet_queue.lock().await.pop()
                {
                    trace!("Spawning flooding task with payload");
                    let state_clone = state.clone();
                    let payload = relay_packet.convert_to_lorawan_phy_payload();
                    tokio::spawn(async move {
                        Self::flooding(state_clone, payload, data_rate, frequency).await;
                    });

                    continue;
                }
            }

            // Next bundle fragment payload
            {
                trace!("Checking for bundle fragment");

                match get_next_payload_from_send_buffer_queue(
                    state.queue_manager.bundle_send_buffer_queue.lock().await,
                    data_rate,
                    &state,
                )
                .await
                {
                    Ok(payload) => {
                        let state_clone = state.clone();
                        tokio::spawn(async move {
                            Self::flooding(state_clone, payload, data_rate, frequency).await;
                        });

                        continue;
                    }
                    Err(NextPacketFromSendBufferError::NoSendBufferInQueue) => {}
                    Err(_) => {
                        skip_delay = true;
                        continue;
                    }
                }
            }
        }
    }

    /// Not used.
    fn provide_shutdown_agent(&mut self, _shutdown_agent: ShutdownAgent) {}
}
