use crate::end_device_id::EndDeviceId;
use crate::error::ReceiveBufferError;
use crate::lorawan_protocol::{AnnouncementPayload, Fragment, GpsLocation};
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, HashSet};

/// Buffer to collect announcement fragments.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct AnnouncementReceiveBuffer {
    source: EndDeviceId,
    location: Option<GpsLocation>,
    total_fragments: u8,
    received_fragments: BTreeMap<u8, Vec<EndDeviceId>>,
}

/// After a [`AnnouncementReceiveBuffer`] has received all fragments, a [`CombinedAnnouncement`]
/// can be constructed. It contains all information from the collected fragments.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct CombinedAnnouncement {
    pub source: EndDeviceId,
    pub location: Option<GpsLocation>,
    pub reachable_ids: Vec<EndDeviceId>,
}

impl AnnouncementReceiveBuffer {
    /// Create a new [`AnnouncementReceiveBuffer`] with the source and total amount of fragments.
    pub fn new(source: EndDeviceId, total_fragments: u8) -> Self {
        Self {
            source,
            location: None,
            total_fragments,
            received_fragments: BTreeMap::new(),
        }
    }

    /// Set the location of the [`AnnouncementReceiveBuffer`]. Overwrites any value currently set.
    pub fn set_location(&mut self, location: GpsLocation) {
        self.location = Some(location)
    }

    /// Process an [`AnnouncementPayload`] into the [`AnnouncementReceiveBuffer`].
    ///
    /// # Errors
    ///
    /// Returns an error if the provided payload is not a fragment, the index has already been seen
    /// or if the fragment amount does not match the [`AnnouncementReceiveBuffer`].
    pub fn process_payload(
        &mut self,
        payload: AnnouncementPayload,
    ) -> Result<(), ReceiveBufferError> {
        let AnnouncementPayload {
            fragment,
            reachable_ids,
        } = payload;

        if let Fragment::Yes {
            index,
            total_amount,
        } = fragment
        {
            if total_amount != self.total_fragments {
                return Err(ReceiveBufferError::FragmentAmountDoesNotMatch);
            }
            if let Entry::Vacant(entry) = self.received_fragments.entry(index) {
                entry.insert(reachable_ids);
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

    /// Combine all received fragments into a [`CombinedAnnouncement`].
    ///
    /// # Errors
    ///
    /// Returns an error if not all fragments have been received.
    pub fn combine(mut self) -> Result<CombinedAnnouncement, ReceiveBufferError> {
        if self.missing_fragments().is_some() {
            return Err(ReceiveBufferError::FragmentsMissing);
        }
        let mut reachable_ids = Vec::new();
        for fragment_payload in self.received_fragments.values_mut() {
            reachable_ids.append(fragment_payload)
        }
        Ok(CombinedAnnouncement {
            source: self.source,
            location: self.location,
            reachable_ids,
        })
    }
}
#[allow(clippy::unwrap_used)]
#[cfg(test)]
mod tests {
    use crate::end_device_id::EndDeviceId;
    use crate::lorawan_protocol::{AnnouncementPayload, Fragment, GpsLocation};
    use crate::receive_buffers::announcement::AnnouncementReceiveBuffer;

    #[test]
    fn announcement_receive_buffer() {
        let source = EndDeviceId(0x12345);
        let total_fragments = 2;
        let mut receive_buffer = AnnouncementReceiveBuffer::new(source, total_fragments);

        let location = GpsLocation::new(80.1, 120.1, 3456.1).unwrap();

        let payload = AnnouncementPayload {
            fragment: Fragment::Yes {
                index: 0,
                total_amount: total_fragments,
            },
            reachable_ids: vec![EndDeviceId(0x12345), EndDeviceId(0x23456)],
        };

        let payload2 = AnnouncementPayload {
            fragment: Fragment::Yes {
                index: 1,
                total_amount: total_fragments,
            },
            reachable_ids: vec![EndDeviceId(0x34567), EndDeviceId(0x45678)],
        };

        receive_buffer.process_payload(payload).unwrap();
        receive_buffer.set_location(location.clone());
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
        assert_eq!(
            combined_bundle.reachable_ids,
            [
                EndDeviceId(0x12345),
                EndDeviceId(0x23456),
                EndDeviceId(0x34567),
                EndDeviceId(0x45678)
            ]
        );
        assert_eq!(combined_bundle.location.unwrap(), location);
        assert_eq!(combined_bundle.source, source);
    }
}
