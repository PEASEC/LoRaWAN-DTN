use crate::end_device_id::EndDeviceId;
use crate::error::SendBufferError;
use crate::lorawan_protocol::{
    AnnouncementPayload, Fragment, GpsLocation, LoRaWanProtocol, MessageType,
};
use crate::send_buffers::SendBuffer;
use chirpstack_gwb_integration::downlinks::predefined_parameters::DataRate;
use std::cmp::Ordering;
use std::f64;

/// Buffer containing an Announcement to be sent
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct AnnouncementSendBuffer {
    source: EndDeviceId,
    location: Option<GpsLocation>,
    fragment_index: u8,
    total_fragments: u8,
    end_device_ids_in_first_fragment: u8,
    end_device_ids_per_fragment: u8,
    data_rate: DataRate,
    /// Points to first unprocessed item
    payload_index: usize,
    payload: Vec<EndDeviceId>,
}

impl AnnouncementSendBuffer {
    /// Create a new [`AnnouncementSendBuffer`].
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - the payload is empty.
    /// - there are not enough bytes per fragment to send at least one [`EndDeviceId`] per fragment
    /// (this is more of an theoretical problem and should not happen in normal usage).
    /// - the payload is too big to be sent with the current data rate as the amount of needed
    /// fragments would exceed the maximum fragment index value.
    pub fn new(
        source: EndDeviceId,
        location: Option<GpsLocation>,
        payload: Vec<EndDeviceId>,
        data_rate: DataRate,
    ) -> Result<Self, SendBufferError> {
        if payload.is_empty() {
            return Err(SendBufferError::EmptyPayload);
        }
        let location_bytes: u8 = if location.is_some() { 9 } else { 0 };
        // 1B MsgType + 4B Src + 1B Fragment ( + 9B GPS) = 6 (+ 9)
        let fixed_bytes_without_fragmentation = 6 + location_bytes;
        if usize::from(data_rate.max_usable_payload_size(false))
            >= usize::from(fixed_bytes_without_fragmentation) + (payload.len() * 4)
        {
            return Ok(Self {
                source,
                location,
                fragment_index: 0,
                total_fragments: 1,
                end_device_ids_in_first_fragment: payload.len() as u8,
                data_rate,
                end_device_ids_per_fragment: 0,
                payload_index: 0,
                payload,
            });
        }

        //  1B MsgType + 4B Src + 1B Fragment + 1B Fragment = 7
        let fixed_bytes_per_fragment = 7_u8;
        // Check whether we can send at least one fragment with location.
        // Check whether we can send at least one end device id per fragment.
        if fixed_bytes_per_fragment + location_bytes >= data_rate.max_usable_payload_size(false)
            || fixed_bytes_per_fragment + 4 >= data_rate.max_usable_payload_size(false)
        {
            return Err(SendBufferError::NotEnoughBytesPerFragmentForOnePayload);
        }
        let remaining_bytes_per_fragment =
            data_rate.max_usable_payload_size(false) - fixed_bytes_per_fragment;
        // We do not split individual end device IDs.
        // Amount of end device IDs in the first packet after accounting for the location.
        let end_device_ids_in_first_fragment =
            ((remaining_bytes_per_fragment - location_bytes) as f64 / 4_f64).floor() as u8;
        // Amount of end device IDs in every packet after the first one.
        let end_device_ids_per_fragment =
            (remaining_bytes_per_fragment as f64 / 4_f64).floor() as u8;

        if payload.len() > 63 * usize::from(end_device_ids_per_fragment) {
            return Err(SendBufferError::PayloadTooBig);
        }

        // We need to send payload.len() end device IDs. In the first fragment, we can send
        // end_device_ids_in_first_fragment, then we can send end_device_ids_per_fragment per fragment.
        // Add 1 for the first fragment.
        let total_fragments = ((payload.len() as f64
            - f64::from(end_device_ids_in_first_fragment))
            / f64::from(end_device_ids_per_fragment))
        .ceil()
            + 1.0;
        // 6 bits for fragment counter
        if total_fragments > 64.0 {
            return Err(SendBufferError::NotEnoughBytesPerFragment);
        }
        let total_fragments = total_fragments as u8;
        Ok(Self {
            source,
            location,
            fragment_index: 0,
            total_fragments,
            end_device_ids_in_first_fragment,
            end_device_ids_per_fragment,
            data_rate,
            payload_index: 0,
            payload,
        })
    }
}

impl SendBuffer for AnnouncementSendBuffer {
    fn next_payload(&mut self) -> Result<LoRaWanProtocol, SendBufferError> {
        // Only 1 Fragment or first fragment (include gps)
        if self.total_fragments == 1 {
            self.fragment_index += 1;
            return Ok(LoRaWanProtocol {
                msg_type: MessageType::Announcement {
                    source: self.source,
                    payload: AnnouncementPayload {
                        fragment: Fragment::No,
                        reachable_ids: self.payload.clone(),
                    },
                    location: self.location.clone(),
                },
            });
        }

        let mut reachable_ids = Vec::new();
        match self.fragment_index.cmp(&(self.total_fragments - 1)) {
            // Any fragment before the last
            Ordering::Less => {
                // First fragment, leave space for location, only use end_device_ids_in_first_fragment
                if self.fragment_index == 0 {
                    for _ in 0..self.end_device_ids_in_first_fragment {
                        let Some(end_device_id) = self
                            .payload
                            .get(self.payload_index) else {
                            return Err(SendBufferError::FragmentCountCalculationWrong)
                        };
                        reachable_ids.push(*end_device_id);
                        self.payload_index += 1;
                    }
                } else {
                    for _ in 0..self.end_device_ids_per_fragment {
                        let Some(end_device_id) = self
                            .payload
                            .get(self.payload_index) else {
                            return Err(SendBufferError::FragmentCountCalculationWrong)
                        };
                        reachable_ids.push(*end_device_id);
                        self.payload_index += 1;
                    }
                }
            }
            // Last fragment
            Ordering::Equal => {
                for _ in self.payload_index..self.payload.len() {
                    let Some(end_device_id) = self
                        .payload
                        .get(self.payload_index) else {
                        return Err(SendBufferError::FragmentCountCalculationWrong)
                    };
                    reachable_ids.push(*end_device_id);
                    self.payload_index += 1;
                }
            }
            Ordering::Greater => {
                return Err(SendBufferError::NoRemainingFragments);
            }
        }

        // Add location only in first fragment
        let location = if self.fragment_index == 0 {
            self.location.clone()
        } else {
            None
        };

        self.fragment_index += 1;
        Ok(LoRaWanProtocol {
            msg_type: MessageType::Announcement {
                source: self.source,
                payload: AnnouncementPayload {
                    fragment: Fragment::Yes {
                        index: self.fragment_index - 1,
                        total_amount: self.total_fragments,
                    },
                    reachable_ids,
                },
                location,
            },
        })
    }

    fn remaining_fragments(&self) -> Option<u8> {
        if self.fragment_index < self.total_fragments {
            Some(self.total_fragments - self.fragment_index)
        } else {
            None
        }
    }

    fn total_fragments(&self) -> u8 {
        self.total_fragments
    }

    fn data_rate(&self) -> DataRate {
        self.data_rate
    }
}

#[allow(clippy::unwrap_used)]
#[cfg(test)]
mod tests {
    use crate::end_device_id::EndDeviceId;
    use crate::lorawan_protocol::{AnnouncementPayload, Fragment, GpsLocation, MessageType};
    use crate::send_buffers::announcement::AnnouncementSendBuffer;
    use crate::send_buffers::SendBuffer;
    use chirpstack_gwb_integration::downlinks::predefined_parameters::DataRate;

    #[test]
    fn announcement_send_buffer() {
        let source = EndDeviceId(0x1234);
        let location = GpsLocation::new(80.5, 30.123, 4.1).unwrap();
        // 42x4 bytes = 168
        let initial_payload = vec![
            EndDeviceId(0x1111),
            EndDeviceId(0x2222),
            EndDeviceId(0x3333),
            EndDeviceId(0x4444),
            EndDeviceId(0x5555),
            EndDeviceId(0x6666),
            EndDeviceId(0x7777),
            EndDeviceId(0x8888),
            EndDeviceId(0x9999),
            EndDeviceId(0x1222),
            EndDeviceId(0x1333),
            EndDeviceId(0x1444),
            EndDeviceId(0x1555),
            EndDeviceId(0x1666),
            EndDeviceId(0x1777),
            EndDeviceId(0x1888),
            EndDeviceId(0x1999),
            EndDeviceId(0x2111),
            EndDeviceId(0x2333),
            EndDeviceId(0x2444),
            EndDeviceId(0x2555),
            EndDeviceId(0x2666),
            EndDeviceId(0x2777),
            EndDeviceId(0x2888),
            EndDeviceId(0x2999),
            EndDeviceId(0x3111),
            EndDeviceId(0x3222),
            EndDeviceId(0x3444),
            EndDeviceId(0x3555),
            EndDeviceId(0x3666),
            EndDeviceId(0x3777),
            EndDeviceId(0x3888),
            EndDeviceId(0x3999),
            EndDeviceId(0x4111),
            EndDeviceId(0x4222),
            EndDeviceId(0x4333),
            EndDeviceId(0x4555),
            EndDeviceId(0x4666),
            EndDeviceId(0x4777),
            EndDeviceId(0x4888),
            EndDeviceId(0x4999),
            EndDeviceId(0x5999),
        ];
        // bytes per fragment: 59
        let mut send_buffer = AnnouncementSendBuffer::new(
            source,
            Some(location.clone()),
            initial_payload.clone(),
            DataRate::Eu863_870Dr0,
        )
        .unwrap();
        assert_eq!(send_buffer.source, source);
        assert_eq!(send_buffer.location, Some(location.clone()));
        assert_eq!(send_buffer.fragment_index, 0);
        assert_eq!(send_buffer.total_fragments, 4);
        // 63B - (7B fixed + 9B GPS) = 47B = 11 EndDeviceId
        assert_eq!(send_buffer.end_device_ids_in_first_fragment, 11);
        // 63B - 7B fixed = 56B = 14 EndDeviceId
        assert_eq!(send_buffer.end_device_ids_per_fragment, 14);
        assert_eq!(send_buffer.payload_index, 0);
        assert_eq!(send_buffer.payload, initial_payload);
        // ((41 - 11) / 14) + 1 = 4

        assert_eq!(send_buffer.remaining_fragments(), Some(4));

        let first_payload = send_buffer.next_payload().unwrap();
        assert_eq!(
            first_payload.msg_type,
            MessageType::Announcement {
                source,
                location: Some(location),
                payload: AnnouncementPayload {
                    fragment: Fragment::Yes {
                        index: 0,
                        total_amount: 4
                    },
                    reachable_ids: Vec::from(initial_payload.get(0..11).unwrap())
                }
            }
        );
        assert_eq!(send_buffer.fragment_index, 1);
        assert_eq!(send_buffer.total_fragments, 4);
        assert_eq!(send_buffer.end_device_ids_in_first_fragment, 11);
        assert_eq!(send_buffer.end_device_ids_per_fragment, 14);
        assert_eq!(send_buffer.payload_index, 11);
        assert_eq!(send_buffer.payload, initial_payload);
        assert_eq!(send_buffer.remaining_fragments(), Some(3));

        let second_payload = send_buffer.next_payload().unwrap();
        assert_eq!(
            second_payload.msg_type,
            MessageType::Announcement {
                source,
                location: None,
                payload: AnnouncementPayload {
                    fragment: Fragment::Yes {
                        index: 1,
                        total_amount: 4
                    },
                    reachable_ids: Vec::from(initial_payload.get(11..25).unwrap())
                }
            }
        );
        assert_eq!(send_buffer.fragment_index, 2);
        assert_eq!(send_buffer.total_fragments, 4);
        assert_eq!(send_buffer.end_device_ids_in_first_fragment, 11);
        assert_eq!(send_buffer.end_device_ids_per_fragment, 14);
        assert_eq!(send_buffer.payload_index, 25);
        assert_eq!(send_buffer.payload, initial_payload);
        assert_eq!(send_buffer.remaining_fragments(), Some(2));

        let third_payload = send_buffer.next_payload().unwrap();
        assert_eq!(
            third_payload.msg_type,
            MessageType::Announcement {
                source,
                location: None,
                payload: AnnouncementPayload {
                    fragment: Fragment::Yes {
                        index: 2,
                        total_amount: 4
                    },
                    reachable_ids: Vec::from(initial_payload.get(25..39).unwrap())
                }
            }
        );
        assert_eq!(send_buffer.fragment_index, 3);
        assert_eq!(send_buffer.total_fragments, 4);
        assert_eq!(send_buffer.end_device_ids_in_first_fragment, 11);
        assert_eq!(send_buffer.end_device_ids_per_fragment, 14);
        assert_eq!(send_buffer.payload_index, 39);
        assert_eq!(send_buffer.payload, initial_payload);
        assert_eq!(send_buffer.remaining_fragments(), Some(1));

        let third_payload = send_buffer.next_payload().unwrap();
        assert_eq!(
            third_payload.msg_type,
            MessageType::Announcement {
                source,
                location: None,
                payload: AnnouncementPayload {
                    fragment: Fragment::Yes {
                        index: 3,
                        total_amount: 4
                    },
                    reachable_ids: Vec::from(initial_payload.get(39..).unwrap())
                }
            }
        );
        assert_eq!(send_buffer.fragment_index, 4);
        assert_eq!(send_buffer.total_fragments, 4);
        assert_eq!(send_buffer.end_device_ids_in_first_fragment, 11);
        assert_eq!(send_buffer.end_device_ids_per_fragment, 14);
        assert_eq!(send_buffer.payload_index, 42);
        assert_eq!(send_buffer.payload, initial_payload);
        assert_eq!(send_buffer.remaining_fragments(), None);
    }
}
