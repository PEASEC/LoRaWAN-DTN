use crate::end_device_id::EndDeviceId;
use crate::error::{ReceiveBufferError, TryFromEndpointIdError};
use crate::lorawan_protocol::{BundleConvergencePayload, Fragment};
use crate::receive_buffers::unix_ts_to_dtn_time;
use bp7::flags::BlockControlFlags;
use chrono::{DateTime, Utc};
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, HashSet};
use std::time::Duration;

/// Buffer to collect bundle fragments.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct BundleReceiveBuffer {
    destination: EndDeviceId,
    source: EndDeviceId,
    timestamp: DateTime<Utc>,
    total_fragments: u8,
    received_fragments: BTreeMap<u8, Vec<u8>>,
}

impl BundleReceiveBuffer {
    /// Create a new [`BundleReceiveBuffer`] with the destination, source, timestamp and total amount of fragments.
    pub fn new(
        destination: EndDeviceId,
        source: EndDeviceId,
        timestamp: DateTime<Utc>,
        total_fragments: u8,
    ) -> Self {
        Self {
            destination,
            source,
            timestamp,
            total_fragments,
            received_fragments: BTreeMap::new(),
        }
    }

    /// Process an [`BundleConvergencePayload`] into the [`BundleReceiveBuffer`].
    ///
    /// # Errors
    ///
    /// Returns an error if the provided payload is not a fragment, the index has already been seen
    /// or if the fragment amount does not match the [`BundleConvergencePayload`].
    pub fn process_payload(
        &mut self,
        payload: BundleConvergencePayload,
    ) -> Result<(), ReceiveBufferError> {
        let BundleConvergencePayload { fragment, payload } = payload;

        if let Fragment::Yes {
            index,
            total_amount,
        } = fragment
        {
            if total_amount != self.total_fragments {
                return Err(ReceiveBufferError::FragmentAmountDoesNotMatch);
            }
            if let Entry::Vacant(entry) = self.received_fragments.entry(index) {
                entry.insert(payload);
                Ok(())
            } else {
                Err(ReceiveBufferError::IndexAlreadyReceived)
            }
        } else {
            Err(ReceiveBufferError::NoFragment)
        }
    }

    /// Return the amount of missing fragments of [`None`] if all fragments have been received.
    pub fn missing_fragments(&self) -> Option<usize> {
        if self.received_fragments.len() < usize::from(self.total_fragments) {
            Some(usize::from(self.total_fragments) - self.received_fragments.len())
        } else {
            None
        }
    }

    /// Return a [`HashSet`] containing all received fragments indices. Returns [`None`] if no
    /// fragment has been received.
    pub fn received_fragment_indices(&self) -> Option<HashSet<u8>> {
        if self.received_fragments.is_empty() {
            None
        } else {
            let mut result = HashSet::new();
            for index in self.received_fragments.keys() {
                result.insert(*index);
            }
            Some(result)
        }
    }

    /// Combine all received fragments into a [`CombinedBundle`].
    ///
    /// # Errors
    ///
    /// Returns an error if not all fragments have been received.
    pub fn combine(mut self) -> Result<CombinedBundle, ReceiveBufferError> {
        if self.missing_fragments().is_some() {
            return Err(ReceiveBufferError::FragmentsMissing);
        }
        let mut payload = Vec::new();
        for fragment_payload in self.received_fragments.values_mut() {
            payload.append(fragment_payload)
        }
        Ok(CombinedBundle {
            destination: self.destination,
            source: self.source,
            timestamp: self.timestamp,
            payload,
        })
    }
}

/// After a [`BundleReceiveBuffer`] has received all fragments, a [`CombinedBundle`]
/// can be constructed. It contains all information from the collected fragments.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct CombinedBundle {
    pub destination: EndDeviceId,
    pub source: EndDeviceId,
    pub timestamp: DateTime<Utc>,
    pub payload: Vec<u8>,
}

impl TryFrom<CombinedBundle> for bp7::Bundle {
    type Error = TryFromEndpointIdError;

    /// Try to create a [`bp7::Bundle`] from a  [`CombinedBundle`].
    ///
    /// # Errors
    ///
    /// Returns an error if the CombinedBundle source or destination cannot be converted into a
    /// bp7 Bundle source or destination. Also if the [`bp7::primary::PrimaryBlockBuilder`] cannot
    /// build a [`bp7::primary::PrimaryBlock`]
    fn try_from(combined_bundle: CombinedBundle) -> Result<Self, Self::Error> {
        let primary = bp7::primary::PrimaryBlockBuilder::new()
            .source(combined_bundle.source.try_into()?)
            .destination(combined_bundle.destination.try_into()?)
            .report_to(combined_bundle.source.try_into()?)
            .creation_timestamp(bp7::CreationTimestamp::with_time_and_seq(
                unix_ts_to_dtn_time(combined_bundle.timestamp.timestamp() as u64),
                0,
            ))
            .lifetime(Duration::from_secs(2 * 24 * 60 * 60))
            .build()?;

        let canonical =
            bp7::canonical::new_payload_block(BlockControlFlags::empty(), combined_bundle.payload);

        Ok(bp7::Bundle::new(primary, vec![canonical]))
    }
}

#[allow(clippy::unwrap_used)]
#[cfg(test)]
mod tests {
    use crate::end_device_id::EndDeviceId;
    use crate::lorawan_protocol::{BundleConvergencePayload, Fragment};
    use crate::receive_buffers::bundle::{BundleReceiveBuffer, CombinedBundle};
    use crate::receive_buffers::unix_ts_to_dtn_time;
    use bp7::CanonicalData;
    use chrono::Utc;

    #[test]
    fn bundle_receive_buffer() {
        let source = EndDeviceId(0x12345);
        let destination = EndDeviceId(0x54321);
        let timestamp = Utc::now();
        let total_fragments = 2;
        let mut receive_buffer =
            BundleReceiveBuffer::new(destination, source, timestamp, total_fragments);

        let payload = BundleConvergencePayload {
            fragment: Fragment::Yes {
                index: 0,
                total_amount: total_fragments,
            },
            payload: vec![0xFF; 20],
        };
        let payload2 = BundleConvergencePayload {
            fragment: Fragment::Yes {
                index: 1,
                total_amount: total_fragments,
            },
            payload: vec![0xFF; 20],
        };

        receive_buffer.process_payload(payload).unwrap();
        assert_eq!(receive_buffer.missing_fragments(), Some(1));
        assert!(receive_buffer
            .received_fragment_indices()
            .unwrap()
            .contains(&0));
        assert_eq!(receive_buffer.received_fragment_indices().unwrap().len(), 1);

        receive_buffer.process_payload(payload2).unwrap();
        assert_eq!(receive_buffer.missing_fragments(), None);
        assert!(receive_buffer
            .received_fragment_indices()
            .unwrap()
            .contains(&0));
        assert!(receive_buffer
            .received_fragment_indices()
            .unwrap()
            .contains(&1));
        assert_eq!(receive_buffer.received_fragment_indices().unwrap().len(), 2);

        let combined_bundle = receive_buffer.combine().unwrap();
        assert_eq!(combined_bundle.payload, [0xFF; 40]);
        assert_eq!(combined_bundle.timestamp, timestamp);
        assert_eq!(combined_bundle.destination, destination);
        assert_eq!(combined_bundle.source, source);
    }

    #[test]
    fn combined_bundle_to_bp7_bundle() {
        let destination = EndDeviceId(0x1234);
        let source = EndDeviceId(0x4321);
        let timestamp = Utc::now();

        let combined_bundle = CombinedBundle {
            destination,
            source,
            timestamp,
            payload: vec![0xFF; 20],
        };
        let bp7_bundle: bp7::Bundle = combined_bundle.try_into().unwrap();
        assert_eq!(bp7_bundle.primary.source, source.try_into().unwrap());
        assert_eq!(bp7_bundle.primary.report_to, source.try_into().unwrap());
        assert_eq!(
            bp7_bundle.primary.destination,
            destination.try_into().unwrap()
        );
        assert_eq!(
            bp7_bundle.primary.creation_timestamp.dtntime(),
            unix_ts_to_dtn_time(timestamp.timestamp() as u64)
        );

        if let CanonicalData::Data(buffer) = bp7_bundle.canonicals.first().unwrap().data() {
            assert_eq!(buffer, &vec![0xFF; 20]);
        } else {
            panic!("No data canonical");
        }
    }
}
