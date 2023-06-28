//! Bundle receive buffer.

use crate::end_device_id::EndDeviceId;
use crate::error::{BundleReceiveBufferCombineError, BundleReceiveBufferProcessError};
use crate::lorawan_protocol::{BundleFragmentOffsetHash, BundlePackets};
use crate::receive_buffers::unix_ts_to_dtn_time;
use bp7::flags::{BlockControlFlags, BundleControlFlags};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::Duration;

/// Buffer to collect bundle fragments.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct BundleReceiveBuffer {
    /// Destination.
    destination: EndDeviceId,
    /// Source.
    source: EndDeviceId,
    /// Timestamp.
    timestamp: DateTime<Utc>,
    /// Total fragments of the bundle, custom LoRaWAN protocol level.
    total_fragments: Option<usize>,
    /// Bundle fragment offset, BP7 protocol level.
    bundle_fragment_offset: Option<u64>,
    /// Bundle total application data unit length, BP7 protocol level.
    bundle_total_application_data_unit_length: Option<u64>,
    /// Bundle fragment offset hash, custom LoRaWAN protocol level.
    bundle_fragment_offset_hash: Option<BundleFragmentOffsetHash>,
    /// Collection of received fragments.
    received_fragments: BTreeMap<u8, Vec<u8>>,
}

impl From<&mut dyn BundlePackets> for BundleReceiveBuffer {
    fn from(bundle_fragment: &mut dyn BundlePackets) -> Self {
        let total_fragments = if bundle_fragment.is_end() {
            Some(usize::from(bundle_fragment.fragment_index()))
        } else {
            None
        };
        let received_fragments =
            BTreeMap::from([(bundle_fragment.fragment_index(), bundle_fragment.payload())]);
        Self {
            destination: bundle_fragment.destination(),
            source: bundle_fragment.source(),
            timestamp: bundle_fragment.timestamp(),
            total_fragments,
            bundle_fragment_offset: bundle_fragment.bundle_fragment_offset(),
            bundle_total_application_data_unit_length: bundle_fragment
                .bundle_total_application_data_unit_length(),
            bundle_fragment_offset_hash: bundle_fragment.bundle_fragment_offset_hash(),
            received_fragments,
        }
    }
}

impl BundleReceiveBuffer {
    /// Processes a packet into the receive buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - the destination, source or timestamp of the packet does not match the receive buffers
    /// destination, source or timestamp.
    /// - the fragment index was already received.
    /// - the fragment offset hash does not match the receive buffers fragment offset hash.
    /// - the to process packet is an end packet and an end packet has already been processed before.
    /// - the end packet of a fragmented bundle had no TADUL or fragment offset.
    pub fn process_packet(
        &mut self,
        packet: &mut dyn BundlePackets,
    ) -> Result<(), BundleReceiveBufferProcessError> {
        if packet.destination() != self.destination {
            return Err(BundleReceiveBufferProcessError::DstDoesNotMatch);
        }

        if packet.source() != self.source {
            return Err(BundleReceiveBufferProcessError::SrcDoesNotMatch);
        }

        if packet.timestamp() != self.timestamp {
            return Err(BundleReceiveBufferProcessError::TimestampDoesNotMatch);
        }

        if self
            .received_fragments
            .contains_key(&packet.fragment_index())
        {
            return Err(BundleReceiveBufferProcessError::IndexAlreadyReceived);
        }

        if self.bundle_fragment_offset_hash != packet.bundle_fragment_offset_hash() {
            return Err(BundleReceiveBufferProcessError::FragmentOffsetHashDoesNotMatch);
        }

        if packet.is_end() {
            if self.total_fragments.is_some() {
                return Err(BundleReceiveBufferProcessError::EndIndexAlreadyReceived);
            }
            self.total_fragments = Some(usize::from(packet.fragment_index()) + 1);
            if packet.bundle_total_application_data_unit_length().is_some() {
                if packet.bundle_fragment_offset().is_some() {
                    self.bundle_fragment_offset = packet.bundle_fragment_offset();
                    self.bundle_total_application_data_unit_length =
                        packet.bundle_total_application_data_unit_length();
                } else {
                    return Err(BundleReceiveBufferProcessError::NoFragmentOffset);
                }
            } else {
                return Err(BundleReceiveBufferProcessError::NoTadul);
            }
        }
        self.received_fragments
            .insert(packet.fragment_index(), packet.payload());
        Ok(())
    }

    /// Returns whether the receive buffer has received all packets and the bundle can be reassembled.
    pub fn is_combinable(&self) -> bool {
        if let Some(total_fragments) = self.total_fragments {
            total_fragments == self.received_fragments.len()
        } else {
            false
        }
    }

    /// Combines the collected fragments into a bundle.
    ///
    /// # Errors:
    ///
    /// Returns an error if:
    /// - a fragment is missing.
    /// - the end has not been received.
    /// - the source and destination cannot be converted from [`EndDeviceId`] to
    /// [`EndpointID`](bp7::eid::EndpointID).
    ///
    pub fn combine(mut self) -> Result<bp7::Bundle, BundleReceiveBufferCombineError> {
        if let Some(total_fragments) = self.total_fragments {
            if total_fragments != self.received_fragments.len() {
                return Err(BundleReceiveBufferCombineError::FragmentsMissing);
            }
        } else {
            return Err(BundleReceiveBufferCombineError::EndNotReceived);
        }
        let mut primary_block_builder = bp7::primary::PrimaryBlockBuilder::new()
            .source(self.source.try_into()?)
            .destination(self.destination.try_into()?)
            .report_to(self.source.try_into()?)
            .creation_timestamp(bp7::CreationTimestamp::with_time_and_seq(
                unix_ts_to_dtn_time(self.timestamp.timestamp().unsigned_abs()),
                0,
            ))
            .lifetime(Duration::from_secs(2 * 24 * 60 * 60));
        let payload = self
            .received_fragments
            .values_mut()
            .fold(Vec::new(), |mut acc, data| {
                acc.append(data);
                acc
            });
        if self.bundle_fragment_offset_hash.is_some() {
            if let Some(bundle_fragment_offset) = self.bundle_fragment_offset {
                if let Some(bundle_total_application_data_unit_length) =
                    self.bundle_total_application_data_unit_length
                {
                    primary_block_builder = primary_block_builder
                        .fragmentation_offset(bundle_fragment_offset)
                        .total_data_length(bundle_total_application_data_unit_length)
                        .bundle_control_flags(BundleControlFlags::BUNDLE_IS_FRAGMENT.bits());
                }
            }
        }
        let primary_block = primary_block_builder.build()?;

        let canonical = bp7::canonical::new_payload_block(BlockControlFlags::empty(), payload);

        Ok(bp7::Bundle::new(primary_block, vec![canonical]))
    }
}
