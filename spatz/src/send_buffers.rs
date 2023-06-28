//! Send buffers.

mod bundle;

use crate::error::SendBufferError;
use crate::lorawan_protocol::LoRaWanPacket;
pub use bundle::BundleSendBuffer;
use chirpstack_gwb_integration::downlinks::predefined_parameters::DataRate;

/// Trait for all send buffers.
pub trait SendBuffer {
    /// Returns the next packet to be sent at the supplied data rate.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The payload was already completely consumed.
    fn next_packet(
        &mut self,
        data_rate: DataRate,
    ) -> Result<Box<dyn LoRaWanPacket>, SendBufferError>;

    /// Returns whether the send buffer has produced all available packets and is empty.
    fn is_empty(&self) -> bool;
}
