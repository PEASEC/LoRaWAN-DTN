//! The custom LoRaWAN protocol.

mod location_encoding;
mod parser;

pub use location_encoding::{encode_alt, encode_lat, encode_long};
pub use parser::{parse_packet, parse_phy_payload};

use crate::end_device_id::EndDeviceId;
use crate::error::{
    BundleFragmentCreationError, CompleteBundleCreationError, LocationEncodingError,
};
use crate::lorawan_protocol::location_encoding::{decode_alt, decode_lat, decode_long};
use chirpstack_gwb_integration::downlinks::predefined_parameters::DataRate;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::fmt::Debug;
use std::hash::Hash;

/// The overhead per packet: 4B Dst + 4B Src + 4B Timestamp
pub static COMPLETE_BUNDLE_HEADERS_SIZE: usize = 4 + 4 + 4 + 1;
/// The overhead per packet: 4B Dst + 4B Src + 4B Timestamp + 1B Fragment index
pub static BUNDLE_FRAGMENT_HEADERS_SIZE: usize = 4 + 4 + 4 + 1;
/// The overhead per packet: 4B Dst + 4B Src + 4B Timestamp + 1B Fragment index +
/// 8B Bundle fragment offset + 8B TADUL
///
/// TADUL Total Application Data Unit Length
pub static FRAGMENTED_BUNDLE_FRAGMENT_START_HEADERS_SIZE: usize = 4 + 4 + 4 + 1 + 8 + 8;
/// The overhead per packet: 4B Dst + 4B Src + 4B Timestamp + 1B Fragment index + 4B Bundle fragment
/// offset hash
pub static FRAGMENTED_BUNDLE_FRAGMENT_HEADERS_SIZE: usize = 4 + 4 + 4 + 1 + 4;
/// The overhead per packet: 4B Dst + 4B Src + 4B Timestamp + 1B Fragment index + 4B Bundle fragment
/// offset hash
pub static FRAGMENTED_BUNDLE_FRAGMENT_END_HEADERS_SIZE: usize = 4 + 4 + 4 + 1 + 4;
/// The overhead per packet: 4B packet hash + 1 Fragment amount + 1B Fragment index
pub static HOP_2_HOP_HEADERS_SIZE: usize = 4 + 1 + 1;
/// The overhead per packet: 4B Src
pub static LOCAL_ANNOUNCEMENT_NO_GPS_HEADERS_SIZE: usize = 4;
/// The overhead per packet: 4B Src + 3B LAT + 3B LONG + 3B ALT
pub static LOCAL_ANNOUNCEMENT_GPS_HEADERS_SIZE: usize = 4 + 3 + 3 + 3;

/// The LoRaWAN protocol proprietary payload tag.
pub static LO_RA_WAN_PROPRIETARY_TAG: u8 = 0b1110_0000;

/// Type alias for the bundle fragment offset hash.
pub type BundleFragmentOffsetHash = u32;

/// All supported packet types of the custom LoRaWAN protocol.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum PacketType {
    /// Complete bundle.
    CompleteBundle,
    /// Fragment of a bundle.
    BundleFragment,
    /// End fragment of a bundle.
    BundleFragmentEnd,
    /// Fragment of a fragmented bundle.
    FragmentedBundleFragment,
    /// End fragment of a fragmented bundle.
    FragmentedBundleFragmentEnd,
    /// Hop 2 hop fragment.
    Hop2HopFragment,
    /// Local announcement.
    LocalAnnouncement,
}

/// Trait of all LoRaWAN packets of the custom LoRaWAN protocol.
#[typetag::serde(tag = "type")]
pub trait LoRaWanPacket: Debug + Send + Sync {
    /// Creates the bytes representation of the packet. Used to create the phy payload for
    /// a LoRaWAN frame.
    fn convert_to_lorawan_phy_payload(&self) -> Vec<u8>;

    /// Convert the packet to a vector of [`Hop2HopFragment`] with the provided data rate.
    fn convert_to_hop_2_hop_fragments(&self, data_rate: DataRate) -> Vec<Hop2HopFragment> {
        let payload = self.convert_to_lorawan_phy_payload();
        let packet_hash = crc32fast::hash(&payload);
        let bytes_per_packet = data_rate.max_usable_payload_size(false) - HOP_2_HOP_HEADERS_SIZE;

        // Amount of fragments is guaranteed to be less than u8::MAX since a payload can at most be
        // 250 bytes.
        #[allow(clippy::cast_possible_truncation)]
        let total_fragments = self
            .convert_to_lorawan_phy_payload()
            .chunks(bytes_per_packet)
            .len() as u8;

        self.convert_to_lorawan_phy_payload()
            .chunks(bytes_per_packet)
            .enumerate()
            .fold(Vec::new(), |mut acc, payload_chunk| {
                let (fragment_index, payload_slice) = payload_chunk;
                // fragment_index is guaranteed ot be less than total_fragments.
                #[allow(clippy::cast_possible_truncation)]
                let fragment_index = fragment_index as u8;
                acc.push(Hop2HopFragment {
                    packet_hash,
                    total_fragments,
                    fragment_index,
                    payload: payload_slice.to_vec(),
                });
                acc
            })
    }

    /// Returns the [`PacketType`] of the packet.
    fn packet_type(&self) -> PacketType;

    /// Returns the destination of the packet if present.
    fn packet_destination(&self) -> Option<EndDeviceId> {
        None
    }

    /// Used to downcast trait objects.
    fn as_any(&self) -> &dyn Any;

    /// Used to downcast trait objects.
    fn as_any_mut(&mut self) -> &mut dyn Any;

    /// Tries to downcast into a [`BundlePackets`] trait object.
    fn as_bundle_packet(&self) -> Option<&dyn BundlePackets> {
        None
    }

    /// Tries to downcast into a mutable [`BundlePackets`] trait object.
    fn as_bundle_packet_mut(&mut self) -> Option<&mut dyn BundlePackets> {
        None
    }
}

/// Trait of all bundle packets of the custom LoRaWAN protocol.
pub trait BundlePackets: LoRaWanPacket {
    /// Returns the destination.
    fn destination(&self) -> EndDeviceId;
    /// Returns the source.
    fn source(&self) -> EndDeviceId;
    /// Returns the timestamp.
    fn timestamp(&self) -> DateTime<Utc>;
    /// Returns whether the packet is an end packet.
    fn is_end(&self) -> bool;
    /// Returns the fragment index.
    fn fragment_index(&self) -> u8;
    /// Returns the payload.
    fn payload(&self) -> Vec<u8>;
    /// Returns the fragment offset hash if present.
    ///
    /// Only present in fragmented bundle fragments.
    fn bundle_fragment_offset_hash(&self) -> Option<BundleFragmentOffsetHash> {
        None
    }
    /// Returnd the total application data unit length if present.
    ///
    /// Only present in fragmented bundle fragments.
    fn bundle_total_application_data_unit_length(&self) -> Option<u64> {
        None
    }
    /// Returns the fragment offset is present.
    ///
    /// Only present in fragmented bundle fragments.
    fn bundle_fragment_offset(&self) -> Option<u64> {
        None
    }
}

/// Complete bundle packet type.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct CompleteBundle {
    /// Destination.
    destination: EndDeviceId,
    /// Source.
    source: EndDeviceId,
    /// Timestamp.
    timestamp: DateTime<Utc>,
    /// Payload.
    payload: Vec<u8>,
}

impl CompleteBundle {
    /// Creates a new [`CompleteBundle`].
    ///
    /// # Errors
    ///
    /// Returns an error if the payload is too large for the provided data rate.
    pub fn new(
        destination: EndDeviceId,
        source: EndDeviceId,
        timestamp: DateTime<Utc>,
        payload: &mut Vec<u8>,
        data_rate: DataRate,
    ) -> Result<Self, CompleteBundleCreationError> {
        if payload.len() <= data_rate.max_usable_payload_size(false) - COMPLETE_BUNDLE_HEADERS_SIZE
        {
            Ok(Self {
                destination,
                source,
                timestamp,
                payload: payload.drain(..).collect(),
            })
        } else {
            Err(CompleteBundleCreationError::PayloadTooLarge)
        }
    }
}

#[typetag::serde]
impl LoRaWanPacket for CompleteBundle {
    fn convert_to_lorawan_phy_payload(&self) -> Vec<u8> {
        let mut result = vec![LO_RA_WAN_PROPRIETARY_TAG];
        result.push(self.packet_type() as u8);
        result.append(&mut convert_end_device_id_to_bytes(self.destination));
        result.append(&mut convert_end_device_id_to_bytes(self.source));
        result.append(&mut convert_timestamp_to_bytes(&self.timestamp));
        result.append(&mut self.payload.clone());
        result
    }

    fn packet_type(&self) -> PacketType {
        PacketType::CompleteBundle
    }

    fn packet_destination(&self) -> Option<EndDeviceId> {
        Some(self.destination)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn as_bundle_packet(&self) -> Option<&dyn BundlePackets> {
        Some(self)
    }

    fn as_bundle_packet_mut(&mut self) -> Option<&mut dyn BundlePackets> {
        Some(self)
    }
}

impl BundlePackets for CompleteBundle {
    fn destination(&self) -> EndDeviceId {
        self.destination
    }

    fn source(&self) -> EndDeviceId {
        self.source
    }

    fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp
    }

    fn is_end(&self) -> bool {
        true
    }

    fn fragment_index(&self) -> u8 {
        1
    }

    fn payload(&self) -> Vec<u8> {
        self.payload.clone()
    }
}

/// Bundle fragment packet type.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct BundleFragment {
    /// Destination.
    destination: EndDeviceId,
    /// Source.
    source: EndDeviceId,
    /// Timestamp.
    timestamp: DateTime<Utc>,
    /// Whether the fragment is an end fragment.
    is_end: bool,
    /// Fragment index.
    fragment_index: u8,
    /// Payload.
    payload: Vec<u8>,
}

impl BundleFragment {
    /// Creates a new [`BundleFragment`].
    ///
    /// The fragment can be of a bundle of a fragmented bundle.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - the payload is empty.
    /// - the provided payload does not fill the maximum usable payload size for the data rate. This
    /// is only allowed for end packets.
    pub fn new(
        destination: EndDeviceId,
        source: EndDeviceId,
        timestamp: DateTime<Utc>,
        is_end: bool,
        fragment_index: u8,
        payload: &mut Vec<u8>,
        data_rate: DataRate,
    ) -> Result<Self, BundleFragmentCreationError> {
        if payload.is_empty() {
            return Err(BundleFragmentCreationError::PayloadEmpty);
        }
        let payload_size = data_rate.max_usable_payload_size(false) - BUNDLE_FRAGMENT_HEADERS_SIZE;
        if payload_size >= payload.len() && !is_end {
            return Err(BundleFragmentCreationError::PayloadNotFilledCompletely);
        }
        let packet_payload: Vec<u8> = payload.drain(..payload_size).collect();
        Ok(Self {
            destination,
            source,
            timestamp,
            is_end,
            fragment_index,
            payload: packet_payload,
        })
    }
}

#[typetag::serde]
impl LoRaWanPacket for BundleFragment {
    fn convert_to_lorawan_phy_payload(&self) -> Vec<u8> {
        let mut result = vec![LO_RA_WAN_PROPRIETARY_TAG];
        result.push(self.packet_type() as u8);
        result.append(&mut convert_end_device_id_to_bytes(self.destination));
        result.append(&mut convert_end_device_id_to_bytes(self.source));
        result.append(&mut convert_timestamp_to_bytes(&self.timestamp));
        result.push(self.fragment_index);
        result.append(&mut self.payload.clone());
        result
    }

    fn packet_type(&self) -> PacketType {
        if self.is_end {
            PacketType::BundleFragmentEnd
        } else {
            PacketType::BundleFragment
        }
    }

    fn packet_destination(&self) -> Option<EndDeviceId> {
        Some(self.destination)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn as_bundle_packet(&self) -> Option<&dyn BundlePackets> {
        Some(self)
    }

    fn as_bundle_packet_mut(&mut self) -> Option<&mut dyn BundlePackets> {
        Some(self)
    }
}

impl BundlePackets for BundleFragment {
    fn destination(&self) -> EndDeviceId {
        self.destination
    }
    fn source(&self) -> EndDeviceId {
        self.source
    }
    fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp
    }
    fn is_end(&self) -> bool {
        self.is_end
    }
    fn fragment_index(&self) -> u8 {
        self.fragment_index
    }
    fn payload(&self) -> Vec<u8> {
        self.payload.clone()
    }
}

/// Fragmented bundle fragment packet type.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct FragmentedBundleFragment {
    /// Destination.
    destination: EndDeviceId,
    /// Source.
    source: EndDeviceId,
    /// Timestamp.
    timestamp: DateTime<Utc>,
    /// Fragment index.
    fragment_index: u8,
    /// Bundle fragment offset hash.
    bundle_fragment_offset_hash: BundleFragmentOffsetHash,
    /// Payload.
    payload: Vec<u8>,
}

#[typetag::serde]
impl LoRaWanPacket for FragmentedBundleFragment {
    fn convert_to_lorawan_phy_payload(&self) -> Vec<u8> {
        let mut result = vec![LO_RA_WAN_PROPRIETARY_TAG];
        result.push(self.packet_type() as u8);
        result.append(&mut convert_end_device_id_to_bytes(self.destination));
        result.append(&mut convert_end_device_id_to_bytes(self.source));
        result.append(&mut convert_timestamp_to_bytes(&self.timestamp));
        result.push(self.fragment_index);
        result.append(&mut Vec::from(
            self.bundle_fragment_offset_hash.to_le_bytes(),
        ));
        result.append(&mut self.payload.clone());
        result
    }

    fn packet_type(&self) -> PacketType {
        PacketType::FragmentedBundleFragment
    }

    fn packet_destination(&self) -> Option<EndDeviceId> {
        Some(self.destination)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn as_bundle_packet(&self) -> Option<&dyn BundlePackets> {
        Some(self)
    }

    fn as_bundle_packet_mut(&mut self) -> Option<&mut dyn BundlePackets> {
        Some(self)
    }
}

impl BundlePackets for FragmentedBundleFragment {
    fn destination(&self) -> EndDeviceId {
        self.destination
    }
    fn source(&self) -> EndDeviceId {
        self.source
    }
    fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp
    }
    fn is_end(&self) -> bool {
        false
    }
    fn fragment_index(&self) -> u8 {
        self.fragment_index
    }
    fn payload(&self) -> Vec<u8> {
        self.payload.clone()
    }
    fn bundle_fragment_offset_hash(&self) -> Option<BundleFragmentOffsetHash> {
        Some(self.bundle_fragment_offset_hash)
    }
}

/// Fragmented bundle fragment end packet type.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct FragmentedBundleFragmentEnd {
    /// Destination.
    destination: EndDeviceId,
    /// Source.
    source: EndDeviceId,
    /// Timestamp.
    timestamp: DateTime<Utc>,
    /// Fragment index.
    fragment_index: u8,
    /// Bundle fragment offset.
    bundle_fragment_offset: u64,
    /// Bundle total application data unit length.
    bundle_total_application_data_unit_length: u64,
    /// Payload.
    payload: Vec<u8>,
}

#[typetag::serde]
impl LoRaWanPacket for FragmentedBundleFragmentEnd {
    fn convert_to_lorawan_phy_payload(&self) -> Vec<u8> {
        let mut result = vec![LO_RA_WAN_PROPRIETARY_TAG];
        result.push(self.packet_type() as u8);
        result.append(&mut convert_end_device_id_to_bytes(self.destination));
        result.append(&mut convert_end_device_id_to_bytes(self.source));
        result.append(&mut convert_timestamp_to_bytes(&self.timestamp));
        result.push(self.fragment_index);
        result.append(&mut Vec::from(self.bundle_fragment_offset.to_le_bytes()));
        result.append(&mut Vec::from(
            self.bundle_total_application_data_unit_length.to_le_bytes(),
        ));
        result.append(&mut self.payload.clone());
        result
    }

    fn packet_type(&self) -> PacketType {
        PacketType::FragmentedBundleFragmentEnd
    }

    fn packet_destination(&self) -> Option<EndDeviceId> {
        Some(self.destination)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn as_bundle_packet(&self) -> Option<&dyn BundlePackets> {
        Some(self)
    }

    fn as_bundle_packet_mut(&mut self) -> Option<&mut dyn BundlePackets> {
        Some(self)
    }
}

impl BundlePackets for FragmentedBundleFragmentEnd {
    fn destination(&self) -> EndDeviceId {
        self.destination
    }

    fn source(&self) -> EndDeviceId {
        self.source
    }

    fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp
    }

    fn is_end(&self) -> bool {
        true
    }

    fn fragment_index(&self) -> u8 {
        self.fragment_index
    }

    fn payload(&self) -> Vec<u8> {
        self.payload.clone()
    }

    fn bundle_fragment_offset_hash(&self) -> Option<BundleFragmentOffsetHash> {
        Some(crc32fast::hash(&self.bundle_fragment_offset.to_le_bytes()))
    }

    fn bundle_total_application_data_unit_length(&self) -> Option<u64> {
        Some(self.bundle_total_application_data_unit_length)
    }

    fn bundle_fragment_offset(&self) -> Option<u64> {
        Some(self.bundle_fragment_offset)
    }
}

/// Hop 2 hop fragment packet type.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct Hop2HopFragment {
    /// Hash of the split packet.
    packet_hash: u32,
    /// Total fragment amount.
    total_fragments: u8,
    /// Fragment index.
    fragment_index: u8,
    /// Payload.
    payload: Vec<u8>,
}

impl Hop2HopFragment {
    /// Returns the packet hash.
    pub fn packet_hash(&self) -> u32 {
        self.packet_hash
    }
    /// Returns the total fragments amount.
    pub fn total_fragments(&self) -> u8 {
        self.total_fragments
    }
    /// Returns the fragment index.
    pub fn fragment_index(&self) -> u8 {
        self.fragment_index
    }
    /// Returns a reference to the payload.
    pub fn payload_ref(&self) -> &Vec<u8> {
        &self.payload
    }
}

#[typetag::serde]
impl LoRaWanPacket for Hop2HopFragment {
    fn convert_to_lorawan_phy_payload(&self) -> Vec<u8> {
        let mut result = vec![LO_RA_WAN_PROPRIETARY_TAG];
        result.push(self.packet_type() as u8);
        result.append(&mut Vec::from(self.packet_hash.to_le_bytes()));
        result.push(self.total_fragments);
        result.push(self.fragment_index);
        result.append(&mut self.payload.clone());
        result
    }

    fn packet_type(&self) -> PacketType {
        PacketType::Hop2HopFragment
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Local announcement packet type.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct LocalAnnouncement {
    /// The optional location of the sender.
    location: Option<GpsLocation>,
    /// All [`EndDeviceId`] registered to the sender.
    end_device_ids: Vec<EndDeviceId>,
}

impl LocalAnnouncement {
    /// Returns the location.
    pub fn location(&self) -> Option<GpsLocation> {
        self.location
    }
    /// Returns the end devices vector by reference.
    pub fn end_device_ids_ref(&self) -> &Vec<EndDeviceId> {
        &self.end_device_ids
    }
}

#[typetag::serde]
impl LoRaWanPacket for LocalAnnouncement {
    fn convert_to_lorawan_phy_payload(&self) -> Vec<u8> {
        let mut result = vec![LO_RA_WAN_PROPRIETARY_TAG];
        result.push(self.packet_type() as u8);
        if let Some(location) = &self.location {
            result.append(&mut convert_location_to_bytes(location));
        }
        for end_device_id in &self.end_device_ids {
            result.append(&mut convert_end_device_id_to_bytes(*end_device_id));
        }
        result
    }

    fn packet_type(&self) -> PacketType {
        PacketType::LocalAnnouncement
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Encoded GPS location.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct GpsLocation {
    /// Latitude.
    latitude: i32,
    /// Longitude.
    longitude: i32,
    /// Altitude.
    altitude: i32,
}

impl GpsLocation {
    /// Creates a new [`GpsLocation`] from floating point coordinates.
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

    /// Converts the internal i32 representation to floating point representation.
    pub fn as_float_coords(&self) -> (f64, f64, f64) {
        (
            decode_lat(self.latitude),
            decode_long(self.longitude),
            decode_alt(self.altitude),
        )
    }
}

/// Convert a `[EndDeviceId`] to its bytes representation in little endian.
fn convert_end_device_id_to_bytes(end_device_id: EndDeviceId) -> Vec<u8> {
    Vec::from(end_device_id.0.to_le_bytes())
}

/// Create the bytes representation of a timestamp.
fn convert_timestamp_to_bytes(timestamp: &DateTime<Utc>) -> Vec<u8> {
    let timestamp = u32::try_from(timestamp.timestamp())
        .expect("This succeeds until u32 cannot hold the unix timestamp anymore.");
    Vec::from(timestamp.to_le_bytes())
}

/// Create the bytes representation of a [`GpsLocation`].
fn convert_location_to_bytes(location: &GpsLocation) -> Vec<u8> {
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

#[allow(clippy::unwrap_used)]
#[cfg(test)]
mod tests {
    use crate::end_device_id::EndDeviceId;
    use crate::lorawan_protocol::parser::{parse_location, parse_phy_payload};
    use crate::lorawan_protocol::{
        convert_location_to_bytes, BundleFragment, GpsLocation, LoRaWanPacket, LocalAnnouncement,
    };
    use chirpstack_gwb_integration::downlinks::predefined_parameters::DataRate;
    use chrono::{DateTime, NaiveDateTime, Utc};

    #[test]
    fn convert_location_to_bytes_test() {
        let location = GpsLocation {
            latitude: 10,
            longitude: -4003,
            altitude: 123_678,
        };
        let loc_bytes = convert_location_to_bytes(&location);
        let (_, parsed_location) = parse_location(loc_bytes.as_slice()).unwrap();
        assert_eq!(location, parsed_location.unwrap());
    }

    #[test]
    fn convert_bundle_fragment_to_bytes_and_back() {
        let timestamp = DateTime::from_utc(
            NaiveDateTime::from_timestamp_opt(Utc::now().timestamp(), 0).unwrap(),
            Utc,
        );
        let packet = BundleFragment {
            destination: EndDeviceId(0x1122_3344),
            source: EndDeviceId(0x5566_7788),
            timestamp,
            is_end: false,
            fragment_index: 10,
            payload: vec![0xFF; 10],
        };
        let packet_bytes = packet.convert_to_lorawan_phy_payload();
        // 1B MHDR + 1B Packet type +  4B DST + 4B SRC + 4B Timestamp + 1B Fragment + 10 B payload = 25
        assert_eq!(25, packet_bytes.len());
        let parsed_packet = parse_phy_payload(&packet_bytes).unwrap();
        assert_eq!(
            &packet,
            parsed_packet
                .as_any()
                .downcast_ref::<BundleFragment>()
                .unwrap()
        );
    }

    #[test]
    fn convert_announcement_to_bytes_and_back() {
        let packet = LocalAnnouncement {
            location: Some(GpsLocation {
                latitude: 30,
                longitude: -1534,
                altitude: 86432,
            }),
            end_device_ids: vec![EndDeviceId(0x1122_3344), EndDeviceId(0x2233_4455)],
        };
        let packet_bytes = packet.convert_to_lorawan_phy_payload();
        let parse_packet = parse_phy_payload(&packet_bytes).unwrap();
        assert_eq!(
            &packet,
            parse_packet
                .as_any()
                .downcast_ref::<LocalAnnouncement>()
                .unwrap()
        );
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

    #[test]
    fn convert_to_hop2hop_fragments() {
        let timestamp = DateTime::from_utc(
            NaiveDateTime::from_timestamp_opt(Utc::now().timestamp(), 0).unwrap(),
            Utc,
        );
        let packet = BundleFragment {
            destination: EndDeviceId(0x1122_3344),
            source: EndDeviceId(0x5566_7788),
            timestamp,
            is_end: false,
            fragment_index: 10,
            payload: vec![0xFF; 10],
        };
        let packet_hash = crc32fast::hash(&packet.convert_to_lorawan_phy_payload());

        let hop2hop_fragments = packet.convert_to_hop_2_hop_fragments(DataRate::Eu863_870Dr0);
        assert_eq!(hop2hop_fragments.len(), 1);
        assert_eq!(hop2hop_fragments.first().unwrap().packet_hash, packet_hash);
        assert_eq!(hop2hop_fragments.first().unwrap().fragment_index, 0);
        assert_eq!(hop2hop_fragments.first().unwrap().total_fragments, 1);

        let packet = BundleFragment {
            destination: EndDeviceId(0x1122_3344),
            source: EndDeviceId(0x5566_7788),
            timestamp,
            is_end: false,
            fragment_index: 10,
            payload: vec![0xFF; 100],
        };
        let packet_hash = crc32fast::hash(&packet.convert_to_lorawan_phy_payload());

        let hop2hop_fragments = packet.convert_to_hop_2_hop_fragments(DataRate::Eu863_870Dr0);
        assert_eq!(hop2hop_fragments.len(), 3);
        assert_eq!(hop2hop_fragments.first().unwrap().packet_hash, packet_hash);
        assert_eq!(hop2hop_fragments.first().unwrap().fragment_index, 0);
        assert_eq!(hop2hop_fragments.first().unwrap().total_fragments, 3);
    }
}
