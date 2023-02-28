//! Parser to parse physical payloads from LoRaWAN frames.

use crate::end_device_id::EndDeviceId;
use crate::error::{IResult, ProtocolParserError};
use crate::lorawan_protocol::{
    AnnouncementPayload, BundleConvergencePayload, Fragment, GpsLocation, LoRaWanProtocol,
    MessageType,
};
use chrono::{DateTime, Utc};
use nom::branch::alt;
use nom::combinator::{cond, map, map_res, value};
use nom::multi::many1;
use nom::sequence::tuple;
use nom::Err::Failure;
use nom::Finish;
use tracing::{instrument, trace};

/// Helper to differentiate between different message types.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ProtocolMessageTypeParserHelper {
    Bundle,
    Announcement,
}

/// Parse proprietary tag: 0b111
fn parse_proprietary_tag(input: (&[u8], usize)) -> IResult<(&[u8], usize), u8> {
    trace!("Parsing proprietary tag");
    nom::bits::complete::tag::<_, _, _, ProtocolParserError>(0b111, 3_usize)(input)
        .map_err(|_| Failure(ProtocolParserError::NoProprietaryTag))
}

/// Parse version tag: 0b00
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

/// Parse the message type.
fn parse_msg_type(input: &[u8]) -> IResult<&[u8], ProtocolMessageTypeParserHelper> {
    trace!("Parsing message type");
    let bundle_msg_tag =
        nom::bits::complete::tag::<_, _, _, ProtocolParserError>(0b0000_0000_u8, 8_usize);
    let announcement_msg_tag = nom::bits::complete::tag(0b0000_0001_u8, 8_usize);
    nom::bits::bits::<_, _, _, _, _>(alt((
        value(ProtocolMessageTypeParserHelper::Bundle, bundle_msg_tag),
        value(
            ProtocolMessageTypeParserHelper::Announcement,
            announcement_msg_tag,
        ),
    )))(input)
    .map_err(|_: nom::Err<_>| Failure(ProtocolParserError::UnknownMsgType))
}

/// Parse the end device id.
fn parse_end_device_id(input: &[u8]) -> IResult<&[u8], EndDeviceId> {
    trace!("Parsing end device ID");
    map(nom::bytes::complete::take(4_usize), |bytes: &[u8]| {
        EndDeviceId(u32::from_le_bytes(<[u8; 4]>::try_from(bytes).expect(
            "We take four bytes with nom, this conversion will not fail.",
        )))
    })(input)
}

/// Parse the unix timestamp from 4 bytes as u32.
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

/// Parse the first bit in the input as bool.
///
/// Takes one bit and returns true if 1, false if 0.
fn parse_bit_flag(input: (&[u8], usize)) -> IResult<(&[u8], usize), bool> {
    map(nom::bits::complete::take(1_usize), |bit: u8| bit > 0)(input)
}

/// Takes seven bits and parses into an u8.
fn parse_7_bits_into_u8(input: (&[u8], usize)) -> IResult<(&[u8], usize), u8> {
    nom::bits::complete::take(7_usize)(input)
}

/// Takes six bits and parses into an u8.
fn parse_6_bits_into_u8(input: (&[u8], usize)) -> IResult<(&[u8], usize), u8> {
    nom::bits::complete::take(6_usize)(input)
}

/// Takes eight bits and parses into an u8.
fn parse_8_bits_into_u8(input: (&[u8], usize)) -> IResult<(&[u8], usize), u8> {
    nom::bits::complete::take(8_usize)(input)
}

/// Parse the bundle convergence headers (index, count).
fn parse_convergence_fragment(input: (&[u8], usize)) -> IResult<(&[u8], usize), Fragment> {
    trace!("Parsing bundle convergence fragment");
    let (input, is_fragment) = parse_bit_flag(input)?;
    let (input, fragment_index) = cond(is_fragment, parse_7_bits_into_u8)(input)?;
    let (input, fragment_count) = cond(is_fragment, parse_8_bits_into_u8)(input)?;

    if is_fragment {
        let fragment_index = fragment_index.expect("Is Some(...) if is_fragment is true");
        let fragment_total_amount = fragment_count.expect("Is Some(...) if is_fragment is true");
        if fragment_index >= fragment_total_amount {
            return Err(Failure(ProtocolParserError::FragmentIndexBiggerThanTotal));
        }
        Ok((
            input,
            Fragment::Yes {
                index: fragment_index,
                total_amount: fragment_total_amount,
            },
        ))
    } else {
        Ok((input, Fragment::No))
    }
}

/// Parse the announcement headers (index, count).
fn parse_announcement_fragment(input: (&[u8], usize)) -> IResult<(&[u8], usize), (Fragment, bool)> {
    trace!("Parsing announcement fragment");
    let (input, is_fragment) = parse_bit_flag(input)?;
    let (input, has_location) = parse_bit_flag(input)?;
    let (input, fragment_index) = cond(is_fragment, parse_6_bits_into_u8)(input)?;
    let (input, fragment_count) = cond(is_fragment, parse_8_bits_into_u8)(input)?;

    if is_fragment {
        let fragment_index = fragment_index.expect("Is Some(...) if is_fragment is true");
        let fragment_total_amount = fragment_count.expect("Is Some(...) if is_fragment is true");
        if fragment_index >= fragment_total_amount {
            return Err(Failure(ProtocolParserError::FragmentIndexBiggerThanTotal));
        }
        Ok((
            input,
            (
                Fragment::Yes {
                    index: fragment_index,
                    total_amount: fragment_total_amount,
                },
                has_location,
            ),
        ))
    } else {
        Ok((input, (Fragment::No, has_location)))
    }
}

/// A wrapper for [`parse_announcement_fragment`].
/// Wraps the conversion from byte parser to bit parser and back.
fn parse_announcement_fragment_wrapper(input: &[u8]) -> IResult<&[u8], (Fragment, bool)> {
    nom::bits::bits(parse_announcement_fragment)(input)
}

/// A wrapper for [`parse_convergence_fragment`].
/// Wraps the conversion from byte parser to bit parser and back.
fn parse_convergence_fragment_wrapper(input: &[u8]) -> IResult<&[u8], Fragment> {
    nom::bits::bits(parse_convergence_fragment)(input)
}

/// Parse the location.
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

/// Parse one or more end device IDs.
fn parse_multiple_end_device_ids(input: &[u8]) -> IResult<&[u8], Vec<EndDeviceId>> {
    trace!("Parsing multiple end device IDs");
    many1(parse_end_device_id)(input)
}

/// Parse a bundle.
fn parse_bundle(input: &[u8]) -> Result<LoRaWanProtocol, ProtocolParserError> {
    trace!("Parsing bundle");
    let (input, destination) = parse_end_device_id(input).finish()?;
    let (input, source) = parse_end_device_id(input).finish()?;
    let (input, timestamp) = parse_timestamp(input).finish()?;
    let (input, fragment) = parse_convergence_fragment_wrapper(input).finish()?;
    let payload = BundleConvergencePayload {
        fragment,
        payload: Vec::from(input),
    };
    Ok(LoRaWanProtocol {
        msg_type: MessageType::Bundle {
            destination,
            source,
            timestamp,
            payload,
        },
    })
}

/// Parse an announcement.
fn parse_announcement(input: &[u8]) -> Result<LoRaWanProtocol, ProtocolParserError> {
    trace!("Parsing announcement");
    let (input, source) = parse_end_device_id(input).finish()?;
    let (input, (fragment, has_location)) = parse_announcement_fragment_wrapper(input).finish()?;
    let (input, location) = if has_location {
        match fragment {
            Fragment::Yes { index: 1, .. } => parse_location(input).finish()?,
            Fragment::Yes { .. } => (input, None),
            Fragment::No => (input, None),
        }
    } else {
        (input, None)
    };
    let (_, reachable_ids) = parse_multiple_end_device_ids(input).finish()?;
    let payload = AnnouncementPayload {
        fragment,
        reachable_ids,
    };
    Ok(LoRaWanProtocol {
        msg_type: MessageType::Announcement {
            source,
            location,
            payload,
        },
    })
}

/// Parse the phy payload of a LoRaWAN frame.
#[instrument(skip(payload))]
pub fn parse_phy_payload(payload: Vec<u8>) -> Result<LoRaWanProtocol, ProtocolParserError> {
    trace!("Entering phy payload parsing");
    let input = payload.as_slice();
    let (input, _) = parse_mac_header(input).finish()?;
    let (input, msg_type_helper) = parse_msg_type(input).finish()?;
    match msg_type_helper {
        ProtocolMessageTypeParserHelper::Bundle => parse_bundle(input),
        ProtocolMessageTypeParserHelper::Announcement => parse_announcement(input),
    }
}

#[allow(clippy::unwrap_used)]
#[cfg(test)]
mod tests {
    use crate::end_device_id::EndDeviceId;
    use crate::error::ProtocolParserError;
    use crate::lorawan_protocol::parser::{
        parse_announcement, parse_bundle, parse_convergence_fragment_wrapper, parse_end_device_id,
        parse_location, parse_mac_header, parse_msg_type, parse_timestamp,
        ProtocolMessageTypeParserHelper,
    };
    use crate::lorawan_protocol::{
        AnnouncementPayload, BundleConvergencePayload, Fragment, GpsLocation, LoRaWanProtocol,
        MessageType,
    };
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
    fn parse_msg_type_bundle() {
        let msg_type = [0b0000_0000_u8];
        let (_, result) = parse_msg_type(&msg_type).unwrap();
        assert_eq!(ProtocolMessageTypeParserHelper::Bundle, result);
    }

    #[test]
    fn parse_msg_type_announcement() {
        let msg_type = [0b0000_0001_u8];
        let (_, result) = parse_msg_type(&msg_type).unwrap();
        assert_eq!(ProtocolMessageTypeParserHelper::Announcement, result);
    }

    #[test]
    fn parse_msg_type_bogus() {
        let msg_type = [0b0000_0010_u8];
        assert_eq!(
            Err(nom::Err::Failure(ProtocolParserError::UnknownMsgType)),
            parse_msg_type(&msg_type)
        );
        let msg_type = [0b0000_0011_u8];
        assert_eq!(
            Err(nom::Err::Failure(ProtocolParserError::UnknownMsgType)),
            parse_msg_type(&msg_type)
        );
        let msg_type = [0b0000_0100_u8];
        assert_eq!(
            Err(nom::Err::Failure(ProtocolParserError::UnknownMsgType)),
            parse_msg_type(&msg_type)
        );
        let msg_type = [0b0000_0101_u8];
        assert_eq!(
            Err(nom::Err::Failure(ProtocolParserError::UnknownMsgType)),
            parse_msg_type(&msg_type)
        );
        let msg_type = [0b0000_0110_u8];
        assert_eq!(
            Err(nom::Err::Failure(ProtocolParserError::UnknownMsgType)),
            parse_msg_type(&msg_type)
        );
        let msg_type = [0b0000_0111_u8];
        assert_eq!(
            Err(nom::Err::Failure(ProtocolParserError::UnknownMsgType)),
            parse_msg_type(&msg_type)
        );
        let msg_type = [0b0000_1000_u8];
        assert_eq!(
            Err(nom::Err::Failure(ProtocolParserError::UnknownMsgType)),
            parse_msg_type(&msg_type)
        );
        let msg_type = [0b0000_1001_u8];
        assert_eq!(
            Err(nom::Err::Failure(ProtocolParserError::UnknownMsgType)),
            parse_msg_type(&msg_type)
        );
        let msg_type = [0b0000_1010_u8];
        assert_eq!(
            Err(nom::Err::Failure(ProtocolParserError::UnknownMsgType)),
            parse_msg_type(&msg_type)
        );
        let msg_type = [0b0000_1011_u8];
        assert_eq!(
            Err(nom::Err::Failure(ProtocolParserError::UnknownMsgType)),
            parse_msg_type(&msg_type)
        );
        let msg_type = [0b0000_1100_u8];
        assert_eq!(
            Err(nom::Err::Failure(ProtocolParserError::UnknownMsgType)),
            parse_msg_type(&msg_type)
        );
        let msg_type = [0b0000_1101_u8];
        assert_eq!(
            Err(nom::Err::Failure(ProtocolParserError::UnknownMsgType)),
            parse_msg_type(&msg_type)
        );
        let msg_type = [0b0000_1110_u8];
        assert_eq!(
            Err(nom::Err::Failure(ProtocolParserError::UnknownMsgType)),
            parse_msg_type(&msg_type)
        );
        let msg_type = [0b0000_1111_u8];
        assert_eq!(
            Err(nom::Err::Failure(ProtocolParserError::UnknownMsgType)),
            parse_msg_type(&msg_type)
        );

        let msg_type = [0b0100_0000_u8];
        assert_eq!(
            Err(nom::Err::Failure(ProtocolParserError::UnknownMsgType)),
            parse_msg_type(&msg_type)
        );
    }

    #[test]
    fn parse_end_device_id_success() {
        let end_device_id = [0xFF, 0xFF, 0xFF, 0xFF, 0x00];
        let (rest, result) = parse_end_device_id(&end_device_id).unwrap();
        assert_eq!(EndDeviceId(u32::MAX), result);
        assert_eq!(rest.len(), 1)
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
    fn parse_convergence_fragment_success() {
        let fragment = [0b1000_0000, 200];
        let (_, bundle_fragment) = parse_convergence_fragment_wrapper(&fragment).unwrap();
        assert_eq!(
            Fragment::Yes {
                index: 0,
                total_amount: 200
            },
            bundle_fragment
        );
        let fragment = [0b1000_0001, 200];
        let (_, bundle_fragment) = parse_convergence_fragment_wrapper(&fragment).unwrap();
        assert_eq!(
            Fragment::Yes {
                index: 1,
                total_amount: 200
            },
            bundle_fragment
        );
        let fragment = [0b1111_1111, 200];
        let (_, bundle_fragment) = parse_convergence_fragment_wrapper(&fragment).unwrap();
        assert_eq!(
            Fragment::Yes {
                index: 127,
                total_amount: 200
            },
            bundle_fragment
        );
    }

    #[test]
    fn parse_convergence_fragment_index_bigger_than_total() {
        let fragment = [0b1100_0000, 2];
        let result = parse_convergence_fragment_wrapper(&fragment);
        assert_eq!(
            Err(nom::Err::Failure(
                ProtocolParserError::FragmentIndexBiggerThanTotal
            )),
            result
        );
    }

    #[test]
    fn parse_convergence_fragment_no_fragment_success() {
        let fragment = [0b0000_0000];
        let (_, bundle_fragment) = parse_convergence_fragment_wrapper(&fragment).unwrap();
        assert_eq!(Fragment::No, bundle_fragment);
        let fragment = [0b0000_0001];
        let (_, bundle_fragment) = parse_convergence_fragment_wrapper(&fragment).unwrap();
        assert_eq!(Fragment::No, bundle_fragment);
        let fragment = [0b0111_1111];
        let (_, bundle_fragment) = parse_convergence_fragment_wrapper(&fragment).unwrap();
        assert_eq!(Fragment::No, bundle_fragment);
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
    fn parse_bundle_test() {
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
            bundle.push(byte)
        }
        // Convergence 1. Byte
        bundle.push(0b1000_0001);
        // Convergence 2. Byte
        bundle.push(0b0000_0010);
        // Payload
        bundle.resize(bundle.len() + 10, 0xFF);

        let bundle_slice = bundle.as_slice();
        let parsed_bundle = parse_bundle(bundle_slice).unwrap();
        let expected_bundle = LoRaWanProtocol {
            msg_type: MessageType::Bundle {
                destination: EndDeviceId(0x78563412),
                source: EndDeviceId(0x12345678),
                timestamp: DateTime::from_utc(
                    NaiveDateTime::from_timestamp_opt(now.timestamp(), 0).unwrap(),
                    Utc,
                ),
                payload: BundleConvergencePayload {
                    fragment: Fragment::Yes {
                        index: 1,
                        total_amount: 2,
                    },
                    payload: vec![0xFF; 10],
                },
            },
        };
        assert_eq!(expected_bundle, parsed_bundle);
    }

    #[allow(clippy::vec_init_then_push)]
    #[test]
    fn parse_announcement_test() {
        let mut announcement = Vec::new();
        // Destination
        announcement.push(0x12);
        announcement.push(0x34);
        announcement.push(0x56);
        announcement.push(0x78);
        // Fragment 1. Byte, is fragment & has location
        announcement.push(0b1100_0001);
        // Fragment 2. Byte
        announcement.push(0b0000_0010);
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
        let parse_announcement = parse_announcement(announcement_slice).unwrap();
        let expected_announcement = LoRaWanProtocol {
            msg_type: MessageType::Announcement {
                source: EndDeviceId(0x78563412),
                location: Some(GpsLocation {
                    latitude: -1,
                    longitude: 1,
                    altitude: 0x00001000,
                }),
                payload: AnnouncementPayload {
                    fragment: Fragment::Yes {
                        index: 1,
                        total_amount: 2,
                    },
                    reachable_ids: vec![EndDeviceId(0x44332211), EndDeviceId(0x88776655)],
                },
            },
        };
        assert_eq!(expected_announcement, parse_announcement);
    }
}
