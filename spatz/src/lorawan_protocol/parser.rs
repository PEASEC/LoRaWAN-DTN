//! Parser to parse physical payloads from LoRaWAN frames.

use crate::end_device_id::EndDeviceId;
use crate::error::{IResult, ProtocolParserError};
use crate::lorawan_protocol::{
    BundleFragment, CompleteBundle, FragmentedBundleFragment, FragmentedBundleFragmentEnd,
    GpsLocation, Hop2HopFragment, LoRaWanPacket, LocalAnnouncement, PacketType,
};
use chrono::{DateTime, Utc};
use nom::branch::alt;
use nom::combinator::{map, map_res, value};
use nom::multi::many1;
use nom::sequence::tuple;
use nom::Err::Failure;
use nom::Finish;
use tracing::{instrument, trace};

/// Parses proprietary tag: 0b111
fn parse_proprietary_tag(input: (&[u8], usize)) -> IResult<(&[u8], usize), u8> {
    trace!("Parsing proprietary tag");
    nom::bits::complete::tag::<_, _, _, ProtocolParserError>(0b111, 3_usize)(input)
        .map_err(|_| Failure(ProtocolParserError::NoProprietaryTag))
}

/// Parses version tag: 0b00
fn parse_version_tag(input: (&[u8], usize)) -> IResult<(&[u8], usize), u8> {
    trace!("Parsing version tag");
    nom::bits::complete::tag::<_, _, _, ProtocolParserError>(0b00, 2_usize)(input)
        .map_err(|_| Failure(ProtocolParserError::WrongVersionTag))
}

/// Parse MHDR (MAC header) field (first byte of PHY Payload).
/// Matches 0b111x_xx00 with x being ignored.
fn parse_mac_header(input: &[u8]) -> IResult<&[u8], bool> {
    trace!("Parsing MAC header");
    // Ignore next three bits
    let rfu_tag = nom::bits::complete::take::<_, u8, _, ProtocolParserError>(3_usize);
    nom::bits::bits(value(
        true,
        tuple((parse_proprietary_tag, rfu_tag, parse_version_tag)),
    ))(input)
}

/// Parses a packet type.
fn parse_packet_type(input: &[u8]) -> IResult<&[u8], PacketType> {
    trace!("Parsing packet type");
    let complete_bundle_tag = nom::bits::complete::tag::<_, _, _, ProtocolParserError>(
        PacketType::CompleteBundle as u8,
        8_usize,
    );
    let bundle_fragment_tag = nom::bits::complete::tag::<_, _, _, ProtocolParserError>(
        PacketType::BundleFragment as u8,
        8_usize,
    );
    let bundle_fragment_end_tag = nom::bits::complete::tag::<_, _, _, ProtocolParserError>(
        PacketType::BundleFragmentEnd as u8,
        8_usize,
    );
    let fragmented_bundle_fragment_tag = nom::bits::complete::tag::<_, _, _, ProtocolParserError>(
        PacketType::FragmentedBundleFragment as u8,
        8_usize,
    );
    let fragmented_bundle_fragment_end_tag = nom::bits::complete::tag::<_, _, _, ProtocolParserError>(
        PacketType::FragmentedBundleFragmentEnd as u8,
        8_usize,
    );
    let hop_2_hop_fragment_tag = nom::bits::complete::tag::<_, _, _, ProtocolParserError>(
        PacketType::Hop2HopFragment as u8,
        8_usize,
    );
    let local_announcement_tag = nom::bits::complete::tag::<_, _, _, ProtocolParserError>(
        PacketType::LocalAnnouncement as u8,
        8_usize,
    );

    nom::bits::bits::<_, _, _, _, _>(alt((
        value(PacketType::CompleteBundle, complete_bundle_tag),
        value(PacketType::BundleFragment, bundle_fragment_tag),
        value(PacketType::BundleFragmentEnd, bundle_fragment_end_tag),
        value(
            PacketType::FragmentedBundleFragment,
            fragmented_bundle_fragment_tag,
        ),
        value(
            PacketType::FragmentedBundleFragmentEnd,
            fragmented_bundle_fragment_end_tag,
        ),
        value(PacketType::Hop2HopFragment, hop_2_hop_fragment_tag),
        value(PacketType::LocalAnnouncement, local_announcement_tag),
    )))(input)
    .map_err(|_: nom::Err<_>| Failure(ProtocolParserError::UnknownPacketType))
}

/// Parses an end device id.
fn parse_end_device_id(input: &[u8]) -> IResult<&[u8], EndDeviceId> {
    trace!("Parsing end device ID");
    map(nom::bytes::complete::take(4_usize), |bytes: &[u8]| {
        EndDeviceId(u32::from_le_bytes(<[u8; 4]>::try_from(bytes).expect(
            "We take four bytes with nom, this conversion will not fail.",
        )))
    })(input)
}

/// Parses a unix timestamp from 4 bytes as u32.
fn parse_timestamp(input: &[u8]) -> nom::IResult<&[u8], DateTime<Utc>, ProtocolParserError> {
    trace!("Parsing timestamp");
    map_res(nom::bytes::complete::take(4_usize), |bytes: &[u8]| {
        let unix_timestamp = u32::from_le_bytes(
            <[u8; 4]>::try_from(bytes)
                .expect("We take four bytes with nom, this conversion will not fail."),
        );
        let unix_timestamp = i64::from(unix_timestamp);
        let Some(naive_time) = chrono::naive::NaiveDateTime::from_timestamp_opt(unix_timestamp, 0) else{
            return Err(ProtocolParserError::FromTimestampError);
        };
        Ok(DateTime::from_utc(naive_time, Utc))
    })(input)
}

/// Parses a location.
pub(crate) fn parse_location(input: &[u8]) -> IResult<&[u8], Option<GpsLocation>> {
    trace!("Parsing location");
    let (input, latitude) =
        map_res(nom::bytes::complete::take(3_usize), convert_3_bytes_to_i32)(input)?;
    let (input, longitude) =
        map_res(nom::bytes::complete::take(3_usize), convert_3_bytes_to_i32)(input)?;
    let (input, altitude) =
        map_res(nom::bytes::complete::take(3_usize), convert_3_bytes_to_i32)(input)?;
    Ok((
        input,
        Some(GpsLocation {
            latitude,
            longitude,
            altitude,
        }),
    ))
}

/// Takes 3 bytes and creates a i32 value from them. Preserves signedness.
fn convert_3_bytes_to_i32(input: &[u8]) -> Result<i32, ProtocolParserError> {
    if input.len() != 3 {
        return Err(ProtocolParserError::NotThreeBytes);
    }
    let mut vec = Vec::from(input);
    // Insert because we use little endian.
    vec.insert(0, 0x00);
    let value = i32::from_le_bytes(
        <[u8; 4]>::try_from(vec.as_slice()).expect("Input contains three bytes, we add one here"),
    );
    // Shift to remove the inserted byte needed for conversion into i32.
    let value = value >> 8;
    Ok(value)
}

/// Parses one or more end device IDs.
fn parse_multiple_end_device_ids(input: &[u8]) -> IResult<&[u8], Vec<EndDeviceId>> {
    trace!("Parsing multiple end device IDs");
    many1(parse_end_device_id)(input)
}

/// Parses bytes into a  [`CompleteBundle`].
///
/// # Errors
///
/// Returns an error if any header cannot be parsed.
fn parse_complete_bundle(input: &[u8]) -> Result<CompleteBundle, ProtocolParserError> {
    trace!("Parsing complete bundle");
    let (input, destination) = parse_end_device_id(input).finish()?;
    let (input, source) = parse_end_device_id(input).finish()?;
    let (input, timestamp) = parse_timestamp(input).finish()?;
    Ok(CompleteBundle {
        destination,
        source,
        timestamp,
        payload: Vec::from(input),
    })
}

/// Parses bytes into a  [`BundleFragment`].
///
/// # Errors
///
/// Returns an error if any header cannot be parsed.
fn parse_bundle_fragment(
    input: &[u8],
    is_end: bool,
) -> Result<BundleFragment, ProtocolParserError> {
    trace!("Parsing bundle fragment");
    let (input, destination) = parse_end_device_id(input).finish()?;
    let (input, source) = parse_end_device_id(input).finish()?;
    let (input, timestamp) = parse_timestamp(input).finish()?;
    let (input, fragment_index) = nom::bytes::complete::take(1_usize)(input).finish()?;
    Ok(BundleFragment {
        destination,
        source,
        timestamp,
        is_end,
        fragment_index: u8::from_le_bytes(
            fragment_index
                .try_into()
                .expect("Nom parsed failed to parse 1 byte without returning an error"),
        ),
        payload: Vec::from(input),
    })
}

/// Parses bytes into a  [`FragmentedBundleFragment`].
///
/// # Errors
///
/// Returns an error if any header cannot be parsed.
fn parse_fragmented_bundle_fragment(
    input: &[u8],
) -> Result<FragmentedBundleFragment, ProtocolParserError> {
    trace!("Parsing fragmented bundle fragment");
    let (input, destination) = parse_end_device_id(input).finish()?;
    let (input, source) = parse_end_device_id(input).finish()?;
    let (input, timestamp) = parse_timestamp(input).finish()?;
    let (input, fragment_index) = nom::bytes::complete::take(1_usize)(input).finish()?;
    let (input, bundle_fragment_offset_hash) =
        nom::bytes::complete::take(4_usize)(input).finish()?;
    Ok(FragmentedBundleFragment {
        destination,
        source,
        timestamp,
        fragment_index: u8::from_le_bytes(
            fragment_index
                .try_into()
                .expect("Nom parsed failed to parse 1 byte without returning an error"),
        ),
        bundle_fragment_offset_hash: u32::from_le_bytes(
            bundle_fragment_offset_hash
                .try_into()
                .expect("Nom parsed failed to parse 4 byte without returning an error"),
        ),
        payload: Vec::from(input),
    })
}

/// Parses bytes into a  [`FragmentedBundleFragmentEnd`].
///
/// # Errors
///
/// Returns an error if any header cannot be parsed.
fn parse_fragmented_bundle_fragment_end(
    input: &[u8],
) -> Result<FragmentedBundleFragmentEnd, ProtocolParserError> {
    trace!("Parsing fragmented bundle fragment end");
    let (input, destination) = parse_end_device_id(input).finish()?;
    let (input, source) = parse_end_device_id(input).finish()?;
    let (input, timestamp) = parse_timestamp(input).finish()?;
    let (input, fragment_index) = nom::bytes::complete::take(1_usize)(input).finish()?;
    let (input, bundle_fragment_offset) = nom::bytes::complete::take(8_usize)(input).finish()?;
    let (input, bundle_total_application_data_unit_length) =
        nom::bytes::complete::take(8_usize)(input).finish()?;
    Ok(FragmentedBundleFragmentEnd {
        destination,
        source,
        timestamp,
        fragment_index: u8::from_le_bytes(
            fragment_index
                .try_into()
                .expect("Nom parsed failed to parse 1 byte without returning an error"),
        ),
        bundle_fragment_offset: u64::from_le_bytes(
            bundle_fragment_offset
                .try_into()
                .expect("Nom parsed failed to parse 8 byte without returning an error"),
        ),
        bundle_total_application_data_unit_length: u64::from_le_bytes(
            bundle_total_application_data_unit_length
                .try_into()
                .expect("Nom parsed failed to parse 8 byte without returning an error"),
        ),
        payload: Vec::from(input),
    })
}

/// Parses bytes into a  [`Hop2HopFragment`].
///
/// # Errors
///
/// Returns an error if any header cannot be parsed.
fn parse_hop_2_hop_fragment(input: &[u8]) -> Result<Hop2HopFragment, ProtocolParserError> {
    trace!("Parsing hop 2 hop fragment");
    let (input, frame_hash) = nom::bytes::complete::take(4_usize)(input).finish()?;
    let (input, fragment_amount) = nom::bytes::complete::take(1_usize)(input).finish()?;
    let (input, fragment_index) = nom::bytes::complete::take(1_usize)(input).finish()?;

    Ok(Hop2HopFragment {
        packet_hash: u32::from_le_bytes(
            frame_hash
                .try_into()
                .expect("Nom parsed failed to parse 4 byte without returning an error"),
        ),
        fragment_index: u8::from_le_bytes(
            fragment_index
                .try_into()
                .expect("Nom parsed failed to parse 1 byte without returning an error"),
        ),
        total_fragments: u8::from_le_bytes(
            fragment_amount
                .try_into()
                .expect("Nom parsed failed to parse 1 byte without returning an error"),
        ),
        payload: Vec::from(input),
    })
}

/// Parses bytes into a [`LocalAnnouncement`].
///
/// # Errors
///
/// Returns an error if any header cannot be parsed.
fn parse_local_announcement(input: &[u8]) -> Result<LocalAnnouncement, ProtocolParserError> {
    trace!("Parsing local announcment");
    let (input, location) = if input.len() % 2 == 0 {
        (input, None)
    } else {
        parse_location(input).finish()?
    };
    let (_, payload) = parse_multiple_end_device_ids(input).finish()?;
    Ok(LocalAnnouncement {
        location,
        end_device_ids: payload,
    })
}

/// Parses the phy payload of a LoRaWAN frame.
#[instrument(skip_all)]
pub fn parse_phy_payload(input: &[u8]) -> Result<Box<dyn LoRaWanPacket>, ProtocolParserError> {
    trace!("Entering phy payload parsing");
    let (input, _) = parse_mac_header(input).finish()?;
    parse_packet(input)
}

/// Parses packet data.
///
/// Used to parse reassembled Hop2Hop packets.
pub fn parse_packet(input: &[u8]) -> Result<Box<dyn LoRaWanPacket>, ProtocolParserError> {
    let (input, packet_type_helper) = parse_packet_type(input).finish()?;
    match packet_type_helper {
        PacketType::CompleteBundle => Ok(Box::new(parse_complete_bundle(input)?)),
        PacketType::BundleFragment => Ok(Box::new(parse_bundle_fragment(input, false)?)),
        PacketType::BundleFragmentEnd => Ok(Box::new(parse_bundle_fragment(input, true)?)),
        PacketType::FragmentedBundleFragment => {
            Ok(Box::new(parse_fragmented_bundle_fragment(input)?))
        }
        PacketType::FragmentedBundleFragmentEnd => {
            Ok(Box::new(parse_fragmented_bundle_fragment_end(input)?))
        }
        PacketType::Hop2HopFragment => Ok(Box::new(parse_hop_2_hop_fragment(input)?)),
        PacketType::LocalAnnouncement => Ok(Box::new(parse_local_announcement(input)?)),
    }
}

#[allow(clippy::unwrap_used)]
#[cfg(test)]
mod tests {
    use crate::end_device_id::EndDeviceId;
    use crate::error::ProtocolParserError;
    use crate::lorawan_protocol::parser::{
        parse_complete_bundle, parse_end_device_id, parse_local_announcement, parse_location,
        parse_mac_header, parse_packet_type, parse_timestamp, PacketType,
    };
    use crate::lorawan_protocol::{CompleteBundle, GpsLocation, LocalAnnouncement};
    use chrono::{DateTime, NaiveDateTime, Utc};

    #[test]
    fn parse_proprietary_success() {
        let mhdr = [0b1110_0000_u8];
        let (rest, result) = parse_mac_header(&mhdr).unwrap();
        assert!(result);
        assert_eq!(rest.len(), 0);
    }

    #[test]
    fn parse_proprietary_ignore_rfu() {
        let mhdr = [0b1111_1100_u8];
        let (rest, result) = parse_mac_header(&mhdr).unwrap();
        assert!(result);
        assert_eq!(rest.len(), 0);
    }

    #[test]
    fn parse_proprietary_wrong_version() {
        let mhdr = [0b1110_0010_u8];

        assert_eq!(
            Err(nom::Err::Failure(ProtocolParserError::WrongVersionTag)),
            parse_mac_header(&mhdr)
        );
    }

    #[test]
    fn parse_proprietary_wrong_version_ignore_rfu() {
        let mhdr = [0b1110_1011_u8];
        assert_eq!(
            Err(nom::Err::Failure(ProtocolParserError::WrongVersionTag)),
            parse_mac_header(&mhdr)
        );
    }

    #[test]
    fn parse_packet_type_test() {
        let packet_type = [0b0000_0000_u8];
        let (_, result) = parse_packet_type(&packet_type).unwrap();
        assert_eq!(PacketType::CompleteBundle, result);

        let packet_type = [0b0000_0001_u8];
        let (_, result) = parse_packet_type(&packet_type).unwrap();
        assert_eq!(PacketType::BundleFragment, result);

        let packet_type = [0b0000_0010_u8];
        let (_, result) = parse_packet_type(&packet_type).unwrap();
        assert_eq!(PacketType::BundleFragmentEnd, result);

        let packet_type = [0b0000_0011u8];
        let (_, result) = parse_packet_type(&packet_type).unwrap();
        assert_eq!(PacketType::FragmentedBundleFragment, result);

        let packet_type = [0b0000_0100u8];
        let (_, result) = parse_packet_type(&packet_type).unwrap();
        assert_eq!(PacketType::FragmentedBundleFragmentEnd, result);

        let packet_type = [0b0000_0101u8];
        let (_, result) = parse_packet_type(&packet_type).unwrap();
        assert_eq!(PacketType::Hop2HopFragment, result);

        let packet_type = [0b0000_0110u8];
        let (_, result) = parse_packet_type(&packet_type).unwrap();
        assert_eq!(PacketType::LocalAnnouncement, result);
    }

    #[test]
    fn parse_packet_type_bogus() {
        let packet_type = [0b0010_0010_u8];
        assert_eq!(
            Err(nom::Err::Failure(ProtocolParserError::UnknownPacketType)),
            parse_packet_type(&packet_type)
        );
        let packet_type = [0b0100_0011_u8];
        assert_eq!(
            Err(nom::Err::Failure(ProtocolParserError::UnknownPacketType)),
            parse_packet_type(&packet_type)
        );
        let packet_type = [0b0000_1100_u8];
        assert_eq!(
            Err(nom::Err::Failure(ProtocolParserError::UnknownPacketType)),
            parse_packet_type(&packet_type)
        );
        let packet_type = [0b0000_1101_u8];
        assert_eq!(
            Err(nom::Err::Failure(ProtocolParserError::UnknownPacketType)),
            parse_packet_type(&packet_type)
        );
        let packet_type = [0b0010_0110_u8];
        assert_eq!(
            Err(nom::Err::Failure(ProtocolParserError::UnknownPacketType)),
            parse_packet_type(&packet_type)
        );
        let packet_type = [0b0100_0111_u8];
        assert_eq!(
            Err(nom::Err::Failure(ProtocolParserError::UnknownPacketType)),
            parse_packet_type(&packet_type)
        );
        let packet_type = [0b0000_1000_u8];
        assert_eq!(
            Err(nom::Err::Failure(ProtocolParserError::UnknownPacketType)),
            parse_packet_type(&packet_type)
        );
    }

    #[test]
    fn parse_end_device_id_success() {
        let end_device_id = [0xFF, 0xFF, 0xFF, 0xFF, 0x00];
        let (rest, result) = parse_end_device_id(&end_device_id).unwrap();
        assert_eq!(EndDeviceId(u32::MAX), result);
        assert_eq!(rest.len(), 1);
    }

    #[test]
    fn parse_end_device_id_too_short() {
        let end_device_id = [0xFF, 0xFF];
        assert!(parse_end_device_id(&end_device_id).is_err());
    }

    #[test]
    fn parse_timestamp_success() {
        let now = Utc::now();
        let timestamp = u32::try_from(now.timestamp()).unwrap();
        let timestamp_bytes: [u8; 4] = timestamp.to_le_bytes();
        let (_, parsed_timestamp) = parse_timestamp(&timestamp_bytes).unwrap();

        assert_eq!(now.timestamp(), parsed_timestamp.timestamp());
    }

    #[test]
    fn parse_location_success() {
        let location_bytes = [0x01, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0x00, 0x00, 0x00];
        let (_, location) = parse_location(&location_bytes).unwrap();
        let location = location.unwrap();
        assert_eq!(1, location.latitude);
        assert_eq!(-1, location.longitude);
        assert_eq!(0, location.altitude);
    }

    #[allow(clippy::vec_init_then_push)]
    #[test]
    fn parse_complete_bundle_test() {
        let now = Utc::now();
        let timestamp = u32::try_from(now.timestamp()).unwrap();
        let timestamp_bytes: [u8; 4] = timestamp.to_le_bytes();
        let mut bundle = Vec::new();
        // Destination
        bundle.push(0x12);
        bundle.push(0x34);
        bundle.push(0x56);
        bundle.push(0x78);
        // Source
        bundle.push(0x78);
        bundle.push(0x56);
        bundle.push(0x34);
        bundle.push(0x12);
        // timestamp
        for byte in timestamp_bytes {
            bundle.push(byte);
        }
        // Payload
        bundle.resize(bundle.len() + 10, 0xFF);

        let bundle_slice = bundle.as_slice();
        let parsed_bundle = parse_complete_bundle(bundle_slice).unwrap();
        let expected_bundle = CompleteBundle {
            destination: EndDeviceId(0x7856_3412),
            source: EndDeviceId(0x1234_5678),
            timestamp: DateTime::from_utc(
                NaiveDateTime::from_timestamp_opt(now.timestamp(), 0).unwrap(),
                Utc,
            ),
            payload: vec![0xFF; 10],
        };
        assert_eq!(expected_bundle, parsed_bundle);
    }

    #[allow(clippy::vec_init_then_push)]
    #[test]
    fn parse_local_announcement_test() {
        let mut announcement = Vec::new();
        // LAT
        announcement.push(0xFF);
        announcement.push(0xFF);
        announcement.push(0b1111_1111);
        // LONG
        announcement.push(0x01);
        announcement.push(0x00);
        announcement.push(0x00);
        // ALT
        announcement.push(0x00);
        announcement.push(0x10);
        announcement.push(0x00);
        // Address 1
        announcement.push(0x11);
        announcement.push(0x22);
        announcement.push(0x33);
        announcement.push(0x44);
        // Address 2
        announcement.push(0x55);
        announcement.push(0x66);
        announcement.push(0x77);
        announcement.push(0x88);
        let announcement_slice = announcement.as_slice();
        let parse_announcement = parse_local_announcement(announcement_slice).unwrap();
        let expected_announcement = LocalAnnouncement {
            location: Some(GpsLocation {
                latitude: -1,
                longitude: 1,
                altitude: 0x0000_1000,
            }),
            end_device_ids: vec![EndDeviceId(0x4433_2211), EndDeviceId(0x8877_6655)],
        };
        assert_eq!(expected_announcement, parse_announcement);
    }
}
