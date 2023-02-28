mod location_encoding;
mod parser;

pub use location_encoding::{encode_alt, encode_lat, encode_long};
pub use parser::parse_phy_payload;

use crate::end_device_id::EndDeviceId;
use crate::error::{LocationEncodingError, ProtocolCreationError};
use chrono::{DateTime, Utc};

/// The encapsulating type for the custom LoRaWAN protocol.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct LoRaWanProtocol {
    pub msg_type: MessageType,
}

/// A Bundle convergence payload. Part of the custom LoRaWAM protocol.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct BundleConvergencePayload {
    pub fragment: Fragment,
    pub payload: Vec<u8>,
}

/// Fragment with metadata or singular message.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Fragment {
    Yes { index: u8, total_amount: u8 },
    No,
}

impl Fragment {
    /// Return index or `None`.
    pub fn index(&self) -> Option<u8> {
        match self {
            Fragment::Yes { index, .. } => Some(*index),
            Fragment::No => None,
        }
    }
}

/// All supported message types of the custom LoRaWAN protocol.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum MessageType {
    Bundle {
        destination: EndDeviceId,
        source: EndDeviceId,
        timestamp: DateTime<Utc>,
        payload: BundleConvergencePayload,
    },
    Announcement {
        source: EndDeviceId,
        location: Option<GpsLocation>,
        payload: AnnouncementPayload,
    },
}

/// Encoded GPS location.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct GpsLocation {
    latitude: i32,
    longitude: i32,
    altitude: i32,
}

/// Payload of an announcement message.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct AnnouncementPayload {
    pub fragment: Fragment,
    pub reachable_ids: Vec<EndDeviceId>,
}

impl GpsLocation {
    /// Create a new [`GpsLocation`] from floating point coordinates.
    ///
    /// # Errors
    ///
    /// Returns a error if one of the provided coordinates is out of range.
    pub fn new(lat: f64, long: f64, alt: f64) -> Result<Self, LocationEncodingError> {
        Ok(Self {
            latitude: encode_lat(lat)?,
            longitude: encode_long(long)?,
            altitude: encode_alt(alt)?,
        })
    }
}

impl LoRaWanProtocol {
    /// Convert a `[EndDeviceId`] to its bytes representation in little endian.
    fn convert_end_device_id_to_bytes(end_device_id: EndDeviceId) -> Vec<u8> {
        Vec::from(end_device_id.0.to_le_bytes())
    }

    /// Create the bytes representation of a [`Fragment`].
    fn convert_bundle_fragment_to_bytes(
        fragment: Fragment,
    ) -> Result<Vec<u8>, ProtocolCreationError> {
        match fragment {
            Fragment::Yes {
                index,
                total_amount,
            } => {
                if index > 0b0111_1111 {
                    return Err(ProtocolCreationError::FragmentIndexTooLarge);
                }
                if total_amount > 0b1000_0000 {
                    return Err(ProtocolCreationError::FragmentTotalAmountTooLarge);
                }
                Ok(vec![(index | 0b1000_0000), total_amount])
            }
            Fragment::No => Ok(vec![0b0000_0000]),
        }
    }

    /// Create the bytes representation of an announcement fragment.
    ///
    /// # Errors
    ///
    /// Return an error if:
    /// - the fragment id is larger than the maximum allowed fragment id.
    /// - the total amount of fragments is larger than the maximum.
    fn convert_announcement_fragment_to_bytes(
        fragment: Fragment,
        has_gps: bool,
    ) -> Result<Vec<u8>, ProtocolCreationError> {
        match fragment {
            Fragment::Yes {
                index,
                total_amount,
            } => {
                if index > 0b0011_1111 {
                    return Err(ProtocolCreationError::FragmentIndexTooLarge);
                }
                if total_amount > 0b0100_0000 {
                    return Err(ProtocolCreationError::FragmentTotalAmountTooLarge);
                }
                let flags = if has_gps { 0b1100_0000 } else { 0b1000_0000 };
                Ok(vec![(index | flags), total_amount])
            }
            Fragment::No => Ok(vec![0b0000_0000]),
        }
    }

    /// Create the bytes representation of a timestamp.
    fn convert_timestamp_to_bytes(timestamp: DateTime<Utc>) -> Vec<u8> {
        let timestamp = u32::try_from(timestamp.timestamp())
            .expect("This succeeds until u32 cannot hold the unix timestamp anymore.");
        Vec::from(timestamp.to_le_bytes())
    }

    /// Create the bytes representation of a [`GpsLocation`].
    fn convert_location_to_bytes(location: GpsLocation) -> Vec<u8> {
        let lat_bytes = &location.latitude.to_le_bytes()[..3];
        let long_bytes = &location.longitude.to_le_bytes()[..3];
        let alt_bytes = &location.altitude.to_le_bytes()[..3];
        let mut lat = if location.latitude.is_negative() {
            vec![lat_bytes[0], lat_bytes[1], lat_bytes[2] | 0b1000_0000]
        } else {
            Vec::from(lat_bytes)
        };
        let mut long = if location.longitude.is_negative() {
            vec![long_bytes[0], long_bytes[1], long_bytes[2] | 0b1000_0000]
        } else {
            Vec::from(long_bytes)
        };
        let mut alt = if location.altitude.is_negative() {
            vec![alt_bytes[0], alt_bytes[1], alt_bytes[2] | 0b1000_0000]
        } else {
            Vec::from(alt_bytes)
        };
        lat.append(&mut long);
        lat.append(&mut alt);
        lat
    }

    /// Create the bytes representation of a [`LoRaWanProtocol`]. Used to create the phy payload of
    /// a LoRaWAN frame.
    ///
    /// # Errors
    ///
    /// Returns an error if the bundle or announcement cannot be converted to bytes.
    pub fn convert_to_lorawan_phy_payload(self) -> Result<Vec<u8>, ProtocolCreationError> {
        // 1B MHDR
        let mut result = vec![0b1110_0000];
        match self.msg_type {
            MessageType::Bundle {
                destination,
                source,
                timestamp,
                mut payload,
            } => {
                result.push(0b0000_0000);
                result.append(&mut Self::convert_end_device_id_to_bytes(destination));
                result.append(&mut Self::convert_end_device_id_to_bytes(source));
                result.append(&mut Self::convert_timestamp_to_bytes(timestamp));
                result.append(&mut Self::convert_bundle_fragment_to_bytes(
                    payload.fragment,
                )?);
                result.append(&mut payload.payload);
                Ok(result)
            }
            MessageType::Announcement {
                source,
                location,
                payload,
            } => {
                result.push(0b0000_0001);
                result.append(&mut Self::convert_end_device_id_to_bytes(source));
                result.append(&mut Self::convert_announcement_fragment_to_bytes(
                    payload.fragment,
                    location.is_some(),
                )?);
                if let Some(location) = location {
                    result.append(&mut Self::convert_location_to_bytes(location));
                }
                for end_device_id in payload.reachable_ids {
                    result.append(&mut Self::convert_end_device_id_to_bytes(end_device_id))
                }
                Ok(result)
            }
        }
    }
}

#[allow(clippy::unwrap_used)]
#[cfg(test)]
mod tests {
    use crate::end_device_id::EndDeviceId;
    use crate::lorawan_protocol::parser::{parse_location, parse_phy_payload};
    use crate::lorawan_protocol::{
        AnnouncementPayload, BundleConvergencePayload, Fragment, GpsLocation, LoRaWanProtocol,
        MessageType,
    };
    use chrono::{DateTime, NaiveDateTime, Utc};

    #[test]
    fn convert_location_to_bytes_test() {
        let location = GpsLocation {
            latitude: 10,
            longitude: -4003,
            altitude: 123678,
        };
        let loc_bytes = LoRaWanProtocol::convert_location_to_bytes(location.clone());
        let (_, parsed_location) = parse_location(loc_bytes.as_slice()).unwrap();
        assert_eq!(location, parsed_location.unwrap());
    }

    #[test]
    fn convert_bundle_to_bytes_and_back() {
        let timestamp = DateTime::from_utc(
            NaiveDateTime::from_timestamp_opt(Utc::now().timestamp(), 0).unwrap(),
            Utc,
        );
        let packet = LoRaWanProtocol {
            msg_type: MessageType::Bundle {
                destination: EndDeviceId(0x11223344),
                source: EndDeviceId(0x55667788),
                timestamp,
                payload: BundleConvergencePayload {
                    fragment: Fragment::Yes {
                        index: 10,
                        total_amount: 30,
                    },
                    payload: vec![0xFF; 10],
                },
            },
        };
        let packet_bytes = packet.clone().convert_to_lorawan_phy_payload().unwrap();
        // 1B MHDR + 1B MsgType +  4B DST + 4B SRC + 4B Timestamp + 1B Fragment + 1B Fragment + 10 B payload = 26
        assert_eq!(26, packet_bytes.len());
        let parse_packet = parse_phy_payload(packet_bytes).unwrap();
        assert_eq!(packet, parse_packet);
    }

    #[test]
    fn convert_announcement_to_bytes_and_back() {
        let packet = LoRaWanProtocol {
            msg_type: MessageType::Announcement {
                source: EndDeviceId(0x55667788),
                location: Some(GpsLocation {
                    latitude: 30,
                    longitude: -1534,
                    altitude: 86432,
                }),
                payload: AnnouncementPayload {
                    fragment: Fragment::Yes {
                        index: 1,
                        total_amount: 12,
                    },

                    reachable_ids: vec![EndDeviceId(0x11223344), EndDeviceId(0x22334455)],
                },
            },
        };
        let packet_bytes = packet.clone().convert_to_lorawan_phy_payload().unwrap();
        let parse_packet = parse_phy_payload(packet_bytes).unwrap();
        assert_eq!(packet, parse_packet);
    }

    #[test]
    fn end_device_id_to_endpoint_id_to_end_device_id() {
        let end_device_id = EndDeviceId(0x1234);
        let endpoint_id: bp7::EndpointID = end_device_id.try_into().unwrap();
        let end_device_id_2: EndDeviceId = endpoint_id.try_into().unwrap();
        assert_eq!(end_device_id, end_device_id_2);
    }

    #[test]
    fn endpoint_id_to_end_device_id_to_endpoint_id() {
        let endpoint_id = bp7::EndpointID::with_dtn("//12356").unwrap();

        let end_device_id: EndDeviceId = endpoint_id.clone().try_into().unwrap();

        let endpoint_id2: bp7::EndpointID = end_device_id.try_into().unwrap();
        assert_eq!(endpoint_id, endpoint_id2);
    }
}
