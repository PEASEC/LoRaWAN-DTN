use crate::end_device_id::EndDeviceId;
use crate::error::SendBufferError;
use crate::lorawan_protocol::{BundleConvergencePayload, Fragment, LoRaWanProtocol, MessageType};
use crate::send_buffers::SendBuffer;
use bp7::dtntime::DtnTimeHelpers;
use chirpstack_gwb_integration::downlinks::predefined_parameters::DataRate;
use chrono::{DateTime, NaiveDateTime, Utc};
use std::cmp::Ordering;
use std::f64;

/// Buffer containing an Bundle to be sent
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct BundleSendBuffer {
    destination: EndDeviceId,
    source: EndDeviceId,
    timestamp: DateTime<Utc>,
    fragment_index: u8,
    total_fragments: u8,
    data_rate: DataRate,
    fixed_bytes_per_fragment: u8,
    /// Points to first unprocessed item
    payload_index: usize,
    payload: Vec<u8>,
}

impl BundleSendBuffer {
    /// Create a new [`BundleSendBuffer`].
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
        destination: EndDeviceId,
        source: EndDeviceId,
        timestamp: DateTime<Utc>,
        payload: Vec<u8>,
        data_rate: DataRate,
    ) -> Result<Self, SendBufferError> {
        if payload.is_empty() {
            return Err(SendBufferError::EmptyPayload);
        }
        // 1B MsgType + 4B Dest + 4B Src + 4B timestamp + 1B Fragment  = 14 B
        let fixed_bytes_without_fragmentation = 14;
        if usize::from(data_rate.max_usable_payload_size(false))
            >= fixed_bytes_without_fragmentation + payload.len()
        {
            return Ok(Self {
                destination,
                source,
                timestamp,
                fragment_index: 0,
                total_fragments: 1,
                data_rate,
                fixed_bytes_per_fragment: 0,
                payload_index: 0,
                payload,
            });
        }

        // 1B MsgType + 4B Dest + 4B Src + 4B timestamp + 1B Fragment + 1B Fragment = 15 B
        let fixed_bytes_per_fragment = 15;
        if fixed_bytes_per_fragment >= data_rate.max_usable_payload_size(false) {
            return Err(SendBufferError::NotEnoughBytesPerFragmentForOnePayload);
        }
        let remaining_bytes_per_fragment =
            data_rate.max_usable_payload_size(false) - fixed_bytes_per_fragment;
        if payload.len() > 128 * usize::from(data_rate.max_usable_payload_size(false)) {
            return Err(SendBufferError::PayloadTooBig);
        }
        let total_fragments =
            (payload.len() as f64 / f64::from(remaining_bytes_per_fragment)).ceil();
        // 7 bits for fragment counter
        if total_fragments > 128.0 {
            return Err(SendBufferError::NotEnoughBytesPerFragment);
        }
        let total_fragments = total_fragments as u8;
        Ok(Self {
            destination,
            source,
            timestamp,
            fragment_index: 0,
            total_fragments,
            data_rate,
            fixed_bytes_per_fragment,
            payload_index: 0,
            payload,
        })
    }

    /// Create a [`BundleSendBuffer`] from a [`bp7::Bundle`].
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - there is not data canonical block.
    /// - there are more than one canonical block.
    /// - the source or destination of the bundle cannot pre converted to [`EndDeviceId`].
    /// - the bp7 timestamp cannot be converted into a unix timestamp.
    /// - the [`BundleSendBuffer::new`] error conditions apply.
    pub fn from_bp7_bundle(
        bundle: bp7::Bundle,
        data_rate: DataRate,
    ) -> Result<Self, SendBufferError> {
        let primary = bundle.primary;
        let canonicals = bundle.canonicals;
        let payload = if let Some(canonical) = canonicals.first() {
            if let bp7::canonical::CanonicalData::Data(payload) = canonical.data() {
                payload.to_vec()
            } else {
                return Err(SendBufferError::NoDataCanonical);
            }
        } else {
            return Err(SendBufferError::TooManyCanonicals);
        };

        let source: EndDeviceId = primary.source.try_into()?;
        let destination: EndDeviceId = primary.destination.try_into()?;
        let Some(naive_time) = NaiveDateTime::from_timestamp_opt(primary.creation_timestamp.dtntime().unix() as i64, 0) else{
            return Err(SendBufferError::FromTimestampError);
        };
        let timestamp = DateTime::from_utc(naive_time, Utc);

        BundleSendBuffer::new(destination, source, timestamp, payload, data_rate)
    }
}

impl SendBuffer for BundleSendBuffer {
    fn next_payload(&mut self) -> Result<LoRaWanProtocol, SendBufferError> {
        // Only 1 Fragment
        if self.total_fragments == 1 {
            self.fragment_index += 1;
            return Ok(LoRaWanProtocol {
                msg_type: MessageType::Bundle {
                    destination: self.destination,
                    source: self.source,
                    timestamp: self.timestamp,
                    payload: BundleConvergencePayload {
                        fragment: Fragment::No,
                        payload: self.payload.clone(),
                    },
                },
            });
        }

        let mut payload_buffer = Vec::new();
        match self.fragment_index.cmp(&(self.total_fragments - 1)) {
            // Any fragment before the last
            Ordering::Less => {
                for _ in
                    0..self.data_rate.max_usable_payload_size(false) - self.fixed_bytes_per_fragment
                {
                    let Some(byte) = self
                        .payload
                        .get(self.payload_index) else {
                        return Err(SendBufferError::FragmentCountCalculationWrong)
                    };
                    payload_buffer.push(*byte);
                    self.payload_index += 1;
                }
            }
            // Last fragment
            Ordering::Equal => {
                for _ in self.payload_index..self.payload.len() {
                    let Some(byte) = self
                        .payload
                        .get(self.payload_index) else {
                        return Err(SendBufferError::FragmentCountCalculationWrong)
                    };
                    payload_buffer.push(*byte);
                    self.payload_index += 1;
                }
            }
            Ordering::Greater => {
                return Err(SendBufferError::NoRemainingFragments);
            }
        }
        self.fragment_index += 1;

        Ok(LoRaWanProtocol {
            msg_type: MessageType::Bundle {
                destination: self.destination,
                source: self.source,
                timestamp: self.timestamp,
                payload: BundleConvergencePayload {
                    fragment: Fragment::Yes {
                        index: self.fragment_index - 1,
                        total_amount: self.total_fragments,
                    },
                    payload: payload_buffer,
                },
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
    use crate::lorawan_protocol::{BundleConvergencePayload, Fragment, MessageType};
    use crate::receive_buffers::unix_ts_to_dtn_time;
    use crate::send_buffers::bundle::BundleSendBuffer;
    use crate::send_buffers::SendBuffer;
    use bp7::flags::BlockControlFlags;
    use chirpstack_gwb_integration::downlinks::predefined_parameters::DataRate;
    use chrono::{DateTime, NaiveDateTime, Utc};
    use std::time::Duration;

    #[test]
    fn bundle_send_buffer() {
        let destination = EndDeviceId(0x1234);
        let source = EndDeviceId(0x4321);
        let timestamp = Utc::now();
        let initial_payload = vec![0xFF; 60];
        // 63 - 15 = 48 bytes of payload per fragment
        let data_rate = DataRate::Eu863_870Dr0;
        let mut send_buffer = BundleSendBuffer::new(
            destination,
            source,
            timestamp,
            initial_payload.clone(),
            data_rate,
        )
        .unwrap();
        assert_eq!(send_buffer.destination, destination);
        assert_eq!(send_buffer.source, source);
        assert_eq!(send_buffer.timestamp, timestamp);
        assert_eq!(send_buffer.fragment_index, 0);
        assert_eq!(send_buffer.total_fragments, 2);
        assert_eq!(send_buffer.data_rate, DataRate::Eu863_870Dr0);
        assert_eq!(send_buffer.fixed_bytes_per_fragment, 15);
        assert_eq!(send_buffer.payload_index, 0);
        assert_eq!(send_buffer.payload, initial_payload);

        assert_eq!(send_buffer.remaining_fragments(), Some(2));

        let first_payload = send_buffer.next_payload().unwrap();
        assert_eq!(
            first_payload.msg_type,
            MessageType::Bundle {
                destination,
                source,
                timestamp,
                payload: BundleConvergencePayload {
                    fragment: Fragment::Yes {
                        index: 0,
                        total_amount: 2
                    },
                    payload: Vec::from(initial_payload.get(0..48).unwrap())
                }
            }
        );
        assert_eq!(
            first_payload
                .convert_to_lorawan_phy_payload()
                .unwrap()
                .len(),
            usize::from(data_rate.max_allowed_payload_size(false))
        );
        assert_eq!(send_buffer.fragment_index, 1);
        assert_eq!(send_buffer.total_fragments, 2);
        assert_eq!(send_buffer.data_rate, DataRate::Eu863_870Dr0);
        assert_eq!(send_buffer.fixed_bytes_per_fragment, 15);
        assert_eq!(send_buffer.payload_index, 48);
        assert_eq!(send_buffer.payload, initial_payload);
        assert_eq!(send_buffer.remaining_fragments(), Some(1));

        let second_payload = send_buffer.next_payload().unwrap();
        assert_eq!(
            second_payload.msg_type,
            MessageType::Bundle {
                destination,
                source,
                timestamp,
                payload: BundleConvergencePayload {
                    fragment: Fragment::Yes {
                        index: 1,
                        total_amount: 2
                    },
                    payload: Vec::from(initial_payload.get(48..).unwrap())
                }
            }
        );
        assert_eq!(send_buffer.fragment_index, 2);
        assert_eq!(send_buffer.total_fragments, 2);
        assert_eq!(send_buffer.data_rate, DataRate::Eu863_870Dr0);
        assert_eq!(send_buffer.fixed_bytes_per_fragment, 15);
        assert_eq!(send_buffer.payload_index, 60);
        assert_eq!(send_buffer.payload, initial_payload);
        assert_eq!(send_buffer.remaining_fragments(), None);
    }

    #[test]
    fn from_bp7_bundle_test() {
        let destination = EndDeviceId(0x1234);
        let source = EndDeviceId(0x4321);
        let timestamp = Utc::now();
        let initial_payload = vec![0xFF; 60];
        let data_rate = DataRate::Eu863_870Dr0;

        let primary = bp7::primary::PrimaryBlockBuilder::new()
            .source(source.try_into().unwrap())
            .destination(destination.try_into().unwrap())
            .creation_timestamp(bp7::CreationTimestamp::with_time_and_seq(
                unix_ts_to_dtn_time(timestamp.timestamp() as u64),
                0,
            ))
            .lifetime(Duration::from_secs(2 * 24 * 60 * 60))
            .build()
            .expect("At time of writing, build only checks whether a destination is set");

        let canonical =
            bp7::canonical::new_payload_block(BlockControlFlags::empty(), initial_payload.clone());
        let bp7_bundle = bp7::Bundle::new(primary, vec![canonical]);

        let mut send_buffer = BundleSendBuffer::from_bp7_bundle(bp7_bundle, data_rate).unwrap();

        assert_eq!(send_buffer.destination, destination);
        assert_eq!(send_buffer.source, source);
        assert_eq!(send_buffer.timestamp.timestamp(), timestamp.timestamp());
        assert_eq!(send_buffer.fragment_index, 0);
        assert_eq!(send_buffer.total_fragments, 2);
        assert_eq!(send_buffer.data_rate, DataRate::Eu863_870Dr0);
        assert_eq!(send_buffer.fixed_bytes_per_fragment, 15);
        assert_eq!(send_buffer.payload_index, 0);
        assert_eq!(send_buffer.payload, initial_payload);

        assert_eq!(send_buffer.remaining_fragments(), Some(2));

        let first_payload = send_buffer.next_payload().unwrap();
        assert_eq!(
            first_payload.msg_type,
            MessageType::Bundle {
                destination,
                source,
                timestamp: DateTime::from_utc(
                    NaiveDateTime::from_timestamp_opt(timestamp.timestamp(), 0).unwrap(),
                    Utc,
                ),
                payload: BundleConvergencePayload {
                    fragment: Fragment::Yes {
                        index: 0,
                        total_amount: 2
                    },
                    payload: Vec::from(initial_payload.get(0..48).unwrap())
                }
            }
        );
        assert_eq!(
            first_payload
                .convert_to_lorawan_phy_payload()
                .unwrap()
                .len(),
            usize::from(data_rate.max_allowed_payload_size(false))
        );

        assert_eq!(send_buffer.fragment_index, 1);
        assert_eq!(send_buffer.total_fragments, 2);
        assert_eq!(send_buffer.data_rate, DataRate::Eu863_870Dr0);
        assert_eq!(send_buffer.fixed_bytes_per_fragment, 15);
        assert_eq!(send_buffer.payload_index, 48);
        assert_eq!(send_buffer.payload, initial_payload);
        assert_eq!(send_buffer.remaining_fragments(), Some(1));

        let second_payload = send_buffer.next_payload().unwrap();
        assert_eq!(
            second_payload.msg_type,
            MessageType::Bundle {
                destination,
                source,
                timestamp: DateTime::from_utc(
                    NaiveDateTime::from_timestamp_opt(timestamp.timestamp(), 0).unwrap(),
                    Utc,
                ),
                payload: BundleConvergencePayload {
                    fragment: Fragment::Yes {
                        index: 1,
                        total_amount: 2
                    },
                    payload: Vec::from(initial_payload.get(48..).unwrap())
                }
            }
        );
        assert_eq!(send_buffer.fragment_index, 2);
        assert_eq!(send_buffer.total_fragments, 2);
        assert_eq!(send_buffer.data_rate, DataRate::Eu863_870Dr0);
        assert_eq!(send_buffer.fixed_bytes_per_fragment, 15);
        assert_eq!(send_buffer.payload_index, 60);
        assert_eq!(send_buffer.payload, initial_payload);
        assert_eq!(send_buffer.remaining_fragments(), None);
    }
}
