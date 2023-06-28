//! Routing algorithms.

mod flooding;

pub use flooding::Flooding;

use crate::error::NextPacketFromSendBufferError;
use crate::graceful_shutdown::ShutdownAgent;
use crate::send_buffers::SendBuffer;
use crate::AppState;
use async_trait::async_trait;
use chirpstack_gwb_integration::downlinks::downlink_builder::DownlinkBuilder;
use chirpstack_gwb_integration::downlinks::downlink_item_builder::DownlinkItemBuilder;
use chirpstack_gwb_integration::downlinks::predefined_parameters::{DataRate, Frequency};
use chirpstack_gwb_integration::downlinks::{Downlink, DownlinkItem, ImmediatelyClassC};
use std::sync::Arc;
use tokio::sync::MutexGuard;
use tracing::info;

/// Routing need to be a task running and update itself (async task spawned)
///
/// Routing algorithms need access to:
/// - gateway IDs (have to be unique network wide)
/// - relay packet queue
/// - bundle queue
/// - duty cycle manager
///
/// Returns:
/// - array of [`Downlink<ImmediatelyClassC>`] to be sent
#[async_trait]
pub trait RoutingAlgorithm: Send + Sync {
    /// The asynchronous task to run to use the routing algorithm.
    async fn routing_task(&self, state: Arc<AppState>, shutdown_agent: ShutdownAgent);
    /// Provides a shutdown agent to the routing algorithm.
    ///
    /// The routing algorithm should use the [`ShutdownAgent`] when performing asynchronous tasks
    /// outside of the `routing_task`.
    fn provide_shutdown_agent(&mut self, shutdown_agent: ShutdownAgent);
}

/// Create a [`DownlinkItem<ImmediatelyClassC>`].
///
/// # Errors
///
/// Returns an error if the downlink item builder encountered an error.
fn create_downlink_item(
    payload: Vec<u8>,
    frequency: Frequency,
    data_rate: DataRate,
) -> Result<
    DownlinkItem<ImmediatelyClassC>,
    chirpstack_gwb_integration::error::DownlinkItemBuilderError,
> {
    DownlinkItemBuilder::<ImmediatelyClassC>::new()
        .frequency(frequency)
        .data_rate(data_rate)
        .power(14)
        .phy_payload(payload)
        .board(0)
        .antenna(0)
        .build()
}

/// Create a [`Downlink<ImmediatelyClassC>`].
///
/// # Errors
///
/// Returns an error if the downlink builder encountered an error.
fn create_downlink(
    gateway_id: String,
    downlink_id: u32,
    item: DownlinkItem<ImmediatelyClassC>,
) -> Result<Downlink<ImmediatelyClassC>, chirpstack_gwb_integration::error::DownlinkBuilderError> {
    DownlinkBuilder::new()
        .gateway_id(gateway_id)
        .downlink_id(downlink_id)
        .add_item(item)
        .build()
}

/// Process a send buffer queue. If a payload is available, the payload is processed by the
/// [`process_next_packet`] function.
///
/// # Errors
///
/// Returns an error if:
/// - the send buffer does not contain any more fragments.
/// - there is no send buffer in the queue.
/// - the [`process_next_packet`] function returned an error.
async fn get_next_payload_from_send_buffer_queue(
    mut send_buffer_vec: MutexGuard<'_, Vec<impl SendBuffer>>,
    data_rate: DataRate,
    state: &Arc<AppState>,
) -> Result<Vec<u8>, NextPacketFromSendBufferError> {
    if let Some(entry_ref) = send_buffer_vec.first_mut() {
        if entry_ref.is_empty() {
            send_buffer_vec.pop();
            let err = NextPacketFromSendBufferError::NoRemainingFragments;
            info!(%err);
            Err(err)
        } else {
            let lorawan_packet = entry_ref.next_packet(data_rate)?;
            // Remove empty send buffers after the last packet has been produced.
            if entry_ref.is_empty() {
                send_buffer_vec.pop();
            }
            let phy_payload = lorawan_packet.convert_to_lorawan_phy_payload();
            state.packet_cache.insert(&phy_payload).await?;
            Ok(phy_payload)
        }
    } else {
        let err = NextPacketFromSendBufferError::NoSendBufferInQueue;
        info!(%err);
        Err(err)
    }
}
