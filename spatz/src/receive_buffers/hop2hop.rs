//! Hop2Hop receive buffer.

use crate::error::{
    Hop2HopReceiveBufferCombineError, Hop2HopReceiveBufferCreationError,
    Hop2HopReceiveBufferProcessPacketError,
};
use crate::lorawan_protocol::{parse_packet, Hop2HopFragment, LoRaWanPacket};
use std::collections::BTreeMap;

/// Buffer to collect hop 2 hop fragments.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Hop2HopReceiveBuffer {
    /// Hash of the split packet.
    packet_hash: u32,
    /// Total amount of fragments.
    total_fragments: usize,
    /// Collection of received fragments.
    received_fragments: BTreeMap<u8, Vec<u8>>,
}

impl TryFrom<&mut Hop2HopFragment> for Hop2HopReceiveBuffer {
    type Error = Hop2HopReceiveBufferCreationError;

    fn try_from(hop2hop_fragment: &mut Hop2HopFragment) -> Result<Self, Self::Error> {
        if hop2hop_fragment.fragment_index() >= hop2hop_fragment.total_fragments() {
            return Err(Hop2HopReceiveBufferCreationError::IndexLargerThanTotal);
        }
        let mut received_fragments = BTreeMap::new();
        received_fragments.insert(
            hop2hop_fragment.fragment_index(),
            hop2hop_fragment.payload_ref().clone(),
        );

        Ok(Self {
            packet_hash: hop2hop_fragment.packet_hash(),
            total_fragments: usize::from(hop2hop_fragment.total_fragments()),
            received_fragments,
        })
    }
}

impl Hop2HopReceiveBuffer {
    /// Processes a packet into the receive buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - the packet hash or total fragments amount of the packet does not match the receive buffers
    /// packet hash or total fragments amount.
    /// - the fragment index is larger than the total amount of fragments.
    /// - the fragment index was already received before.
    pub fn process_packet(
        &mut self,
        packet: &mut Hop2HopFragment,
    ) -> Result<(), Hop2HopReceiveBufferProcessPacketError> {
        if packet.packet_hash() != self.packet_hash {
            return Err(Hop2HopReceiveBufferProcessPacketError::HashMismatch);
        }
        if usize::from(packet.total_fragments()) != self.total_fragments {
            return Err(Hop2HopReceiveBufferProcessPacketError::TotalFragmentsMismatch);
        }
        if usize::from(packet.fragment_index()) >= self.total_fragments {
            return Err(Hop2HopReceiveBufferProcessPacketError::IndexLargerThanTotal);
        }
        if self
            .received_fragments
            .contains_key(&packet.fragment_index())
        {
            return Err(Hop2HopReceiveBufferProcessPacketError::IndexAlreadyReceived);
        }
        self.received_fragments
            .insert(packet.fragment_index(), packet.payload_ref().clone());
        Ok(())
    }
    /// Returns whether the receive buffer has received all packets and the original packet can be
    /// reassembled.
    pub fn is_combinable(&self) -> bool {
        self.received_fragments.len() == self.total_fragments
    }

    /// Combines the collected fragments into a packet.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - a fragment is missing.
    /// - the combined packet cannot be parsed.
    pub fn combine(mut self) -> Result<Box<dyn LoRaWanPacket>, Hop2HopReceiveBufferCombineError> {
        if !self.is_combinable() {
            return Err(Hop2HopReceiveBufferCombineError::FragmentsMissing);
        }
        let payload = self
            .received_fragments
            .values_mut()
            .fold(Vec::new(), |mut acc, data| {
                acc.append(data);
                acc
            });
        Ok(parse_packet(&payload)?)
    }
}
