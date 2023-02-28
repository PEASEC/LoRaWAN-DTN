mod announcement;
mod bundle;

pub use announcement::AnnouncementSendBuffer;
pub use bundle::BundleSendBuffer;

use crate::error::SendBufferError;
use crate::lorawan_protocol::LoRaWanProtocol;
use chirpstack_gwb_integration::downlinks::predefined_parameters::DataRate;

/// Common functionality every send buffer implements.
pub trait SendBuffer {
    /// The next payload, if there is one.
    fn next_payload(&mut self) -> Result<LoRaWanProtocol, SendBufferError>;
    /// The amount of fragments remaining or `None` if no fragments remain.
    fn remaining_fragments(&self) -> Option<u8>;
    /// Total number of fragments.
    fn total_fragments(&self) -> u8;
    /// The buffer's data rate.
    fn data_rate(&self) -> DataRate;
    /// Whether part (e.g. at least one fragment) of the [`SendBuffer`] has already been consumed
    /// via [`Self::next_payload()`].
    fn partially_sent(&self) -> bool {
        if let Some(remaining_fragments) = self.remaining_fragments() {
            self.total_fragments() == remaining_fragments
        } else {
            false
        }
    }
}
