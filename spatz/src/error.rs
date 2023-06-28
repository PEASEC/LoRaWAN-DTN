//! All errors used in the spatz code.

use chirpstack_gwb_integration::error::{BandwidthConversionError, SpreadingFactorConversionError};
use nom::error::{FromExternalError, ParseError};
use nom::ErrorConvert;
use std::num::{ParseIntError, TryFromIntError};
use thiserror::Error;

/// Errors returned by the packet cache.
#[derive(Error, Debug, Ord, PartialOrd, PartialEq, Eq)]
pub enum PacketCacheError {
    /// Entry has not timed out yet
    #[error("Entry has not timed out yet")]
    NotTimedOut,
}

/// Errors returned by the protocol parser.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum ProtocolParserError {
    /// Payload has no proprietary tag.
    #[error("Payload has no proprietary tag")]
    NoProprietaryTag,
    /// Payload has wrong version tag.
    #[error("Payload has wrong version tag")]
    WrongVersionTag,
    /// Payload has unknown packet type.
    #[error("Payload has unknown packet type")]
    UnknownPacketType,
    /// Nom error.
    #[error("Nom encountered an error: {0:?}")]
    Nom(nom::error::ErrorKind),
    /// Did not receive three bytes, cannot convert to u32
    #[error("Did not receive three bytes, cannot convert to u32")]
    NotThreeBytes,
    /// Failed to create naive datetime from timestamp.
    #[error("Failed to create naive datetime from timestamp")]
    FromTimestampError,
}

/// Errors occurring when creating a complete bundle packet.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum CompleteBundleCreationError {
    /// The payload is too large and cannot fit into one packet.
    #[error("The payload is too large and cannot fit into one packet")]
    PayloadTooLarge,
}

/// Errors occurring when creating a bundle fragment packet.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum BundleFragmentCreationError {
    /// The provided payload cannot fill the packet payload completely, this is forbidden for all
    /// but end packets.
    #[error(
        "The provided payload cannot fill the packet payload completely, this is forbidden for all
        but end packets"
    )]
    PayloadNotFilledCompletely,
    /// Payload is empty.
    #[error("Payload is empty")]
    PayloadEmpty,
}

/// Errors occurring when encoding a location.
#[allow(clippy::enum_variant_names)]
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum LocationEncodingError {
    /// Could not encode value, was out of range (-90.00 to 90.00).
    #[error("Could not encode value, was out of range (-90.00 to 90.00)")]
    LatOutOfRange,
    /// Could not encode value, was out of range (-180.00 to 180.00)
    #[error("Could not encode value, was out of range (-180.00 to 180.00)")]
    LongOutOfRange,
    /// Could not encode value, was out of range (-83886.00 to 83886.00)
    #[error("Could not encode value, was out of range (-83886.00 to 83886.00)")]
    AltOutOfRange,
}

/// Errors occurring when using the send buffer.
#[derive(Error, Debug, Ord, PartialOrd, PartialEq, Eq)]
pub enum SendBufferError {
    /// The payload was already consumed completely.
    #[error("The payload was already consumed completely")]
    PayloadConsumed,
}

/// Errors occurring when calculating the airtime of a downlink.
#[derive(Error, Debug, PartialEq, Eq)]
pub enum AirtimeCalculationError {
    /// No downlink items.
    #[error("No downlink items")]
    NoItems,
    /// Data extraction error.
    #[error("Data extraction error: {0}")]
    DataRateExtraction(#[from] LoRaModulationExtractionError),
    /// Failed to convert integer.
    #[error("Failed to convert integer: {0}")]
    NumberConversion(#[from] TryFromIntError),
    /// Failed to convert bandwidth.
    #[error("Failed to convert bandwidth: {0}")]
    BandWidthConversion(#[from] BandwidthConversionError),
    /// Failed to convert spreading factor.
    #[error("Failed to convert spreading factor: {0}")]
    SpreadingFactorConversion(#[from] SpreadingFactorConversionError),
}

/// Errors occurring when creating a bundle send buffer.
#[derive(Error, Debug)]
pub enum BundleSendBufferCreationError {
    /// The payload is too large and cannot be sent completely with the lowest data rate.
    #[error("The payload is too large and cannot be sent completely with the lowest data rate")]
    PayloadTooLarge,
}

/// Errors occurring when creating a sub band.
#[derive(Error, Debug, Ord, PartialOrd, PartialEq, Eq)]
pub enum SubBandCreationError {
    /// No matching sub band for frequency.
    #[error("No matching sub band for frequency: {freq}")]
    NoMatchingSubBand {
        /// The provided frequency that could not be matched to a sub band.
        freq: u32,
    },
}

/// Errors occurring when creating a sub band.
#[derive(Error, Debug, Ord, PartialOrd, PartialEq, Eq)]
pub enum NextPacketFromSendBufferError {
    /// SendBuffer does not contain any more fragments.
    #[error("SendBuffer does not contain any more fragments")]
    NoRemainingFragments,
    /// No SendBuffer in SendBuffer queue.
    #[error("No SendBuffer in SendBuffer queue")]
    NoSendBufferInQueue,
    /// Packet cache error.
    #[error("Packet cache error: {0}")]
    PacketCache(#[from] PacketCacheError),
    /// Send buffer error
    #[error("Send buffer error: {0}")]
    SendBuffer(#[from] SendBufferError),
}

/// Errors occurring when consuming duty cycle time.
#[derive(Error, Debug, Ord, PartialOrd, PartialEq, Eq)]
pub enum ConsumeDutyCycleTimeError {
    /// More capacity used than was available.
    #[error("More capacity used than was available")]
    CapacityOverused,
    /// Sub band error.
    #[error("Sub band error: {0}")]
    SubBand(#[from] SubBandCreationError),
}

/// Errors occurring when trying to convert a [`EndDeviceId`](crate::EndDeviceId) into a [`bp7::EndpointID`].
#[derive(Error, Debug)]
pub enum TryFromEndDeviceId {
    /// Not a Dtn address, only Dtn addressing is supported.
    #[error("Not a Dtn address, only Dtn addressing is supported")]
    NoDtnAddress,
    /// Error parsing int.
    #[error("Error parsing int: {0}")]
    ParseInt(#[from] ParseIntError),
}

/// Errors occurring when processing a packet.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum BundleReceiveBufferProcessError {
    /// Packets destination does not match receive buffers destination.
    #[error("Packets destination does not match receive buffers destination")]
    DstDoesNotMatch,
    /// Packets source does not match receive buffers source.
    #[error("Packets source does not match receive buffers source")]
    SrcDoesNotMatch,
    /// Packets timestamp does not match receive buffers timestamp.
    #[error("Packets timestamp does not match receive buffers timestamp")]
    TimestampDoesNotMatch,
    /// Packets fragment offset hash does not match receive buffers fragment offset hash.
    #[error("Packets fragment offset hash does not match receive buffers fragment offset hash")]
    FragmentOffsetHashDoesNotMatch,
    /// A packet with this index has already been received.
    #[error("A packet with this index has already been received")]
    IndexAlreadyReceived,
    /// A packet with an end index has already been received.
    #[error("A packet with an end index has already been received")]
    EndIndexAlreadyReceived,
    /// Fragmented bundle fragment end packet has no TADUL.
    #[error("Fragmented bundle fragment end packet has no TADUL")]
    NoTadul,
    /// Fragmented bundle fragment end packet has no fragment offset.
    #[error("Fragmented bundle fragment end packet has no fragment offset")]
    NoFragmentOffset,
}

/// Errors occurring when trying to create a [`BundleSendBuffer`](crate::send_buffers::BundleSendBuffer) from a [`bp7::Bundle`].
#[derive(Error, Debug)]
pub enum BundleSendBufferConversionError {
    /// Bundle has no payload.
    #[error("Bundle has no payload")]
    NoPayload,
    /// Failed to create naive datetime from timestamp.
    #[error("Failed to create naive datetime from timestamp")]
    TryFromTimestampError,
    /// Endpoint conversion error.
    #[error("Endpoint conversion error: {0}")]
    TryFromEndpointId(#[from] TryFromEndDeviceId),
    /// Bundle send buffer creation error.
    #[error("Bundle send buffer creation error: ")]
    BundleSendBuffer(#[from] BundleSendBufferCreationError),
}

/// Errors occurring when combining the fragments in a [`BundleReceiveBuffer`](crate::receive_buffers::BundleReceiveBuffer).
#[derive(Error, Debug)]
pub enum BundleReceiveBufferCombineError {
    /// No packet indicating the end has been received.
    #[error("No packet indicating the end has been received")]
    EndNotReceived,
    /// Not all fragments have been received.
    #[error("Not all fragments have been received")]
    FragmentsMissing,
    /// Endpoint ID error from bp7.
    #[error("Endpoint ID error from bp7: {0}")]
    EndpointId(#[from] bp7::eid::EndpointIdError),
    /// Primary builder error from bp7.
    #[error("Primary builder error from bp7: {0}")]
    PrimaryBuilder(#[from] bp7::primary::PrimaryBuilderError),
}

/// Errors occurring when extracting the [`LoraModulationInfo`](chirpstack_api::gw::LoraModulationInfo).
#[derive(Error, Debug, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub enum LoRaModulationExtractionError {
    /// No TX info in frame.
    #[error("No TX info in frame")]
    NoTxInfo,
    /// No modulation info in frame.
    #[error("No modulation info in frame")]
    NoModulationInfo,
    /// No LoRa parameters in modulation in frame.
    #[error("No LoRa parameters in modulation in frame")]
    NoLoRaParameters,
}

/// Errors occurring when creating a [`Hop2HopReceiveBuffer`](crate::receive_buffers::Hop2HopReceiveBuffer).
#[derive(Error, Debug, PartialEq, Eq)]
pub enum Hop2HopReceiveBufferCreationError {
    /// Fragment index is larger than total amount of fragments
    #[error("Fragment index is larger than total amount of fragments")]
    IndexLargerThanTotal,
}

/// Errors occurring when processing a Hop2Hop packet fragment.
#[derive(Error, Debug, PartialEq, Eq)]
pub enum Hop2HopReceiveBufferProcessPacketError {
    /// The fragments packet hash does not match.
    #[error("The fragments packet hash does not match")]
    HashMismatch,
    /// The fragments packet total fragment amount does not match.
    #[error("The fragments packet total fragment amount does not match")]
    TotalFragmentsMismatch,
    /// Fragment index is larger than total amount of fragments.
    #[error("Fragment index is larger than total amount of fragments")]
    IndexLargerThanTotal,
    /// A packet with this index has already been received.
    #[error("A packet with this index has already been received")]
    IndexAlreadyReceived,
}

/// Errors occurring when combining the fragments in a [`Hop2HopReceiveBuffer`](crate::receive_buffers::Hop2HopReceiveBuffer).
#[derive(Error, Debug, PartialEq, Eq)]
pub enum Hop2HopReceiveBufferCombineError {
    /// Not all fragment received.
    #[error("Not all fragments received")]
    FragmentsMissing,
    /// Protocol parser error.
    #[error("Protocol parser error: {0}")]
    ProtocolParser(#[from] ProtocolParserError),
}

/// Errors occurring when interacting with the database.
#[derive(Error, Debug)]
pub enum DbError {
    /// Serde_json error
    #[error("Deserializing error from serde_json: {0}")]
    SerdeJson(#[from] serde_json::Error),
    /// Sqlx error
    #[error("Database error form sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),
}

impl ErrorConvert<ProtocolParserError> for ProtocolParserError {
    fn convert(self) -> ProtocolParserError {
        self
    }
}

impl<I> ParseError<I> for ProtocolParserError {
    fn from_error_kind(_: I, kind: nom::error::ErrorKind) -> Self {
        ProtocolParserError::Nom(kind)
    }

    fn append(_: I, _: nom::error::ErrorKind, other: Self) -> Self {
        other
    }
}

impl FromExternalError<&[u8], ProtocolParserError> for ProtocolParserError {
    fn from_external_error(_: &[u8], _: nom::error::ErrorKind, e: ProtocolParserError) -> Self {
        e
    }
}

/// Type alias for packet parsing.
pub type IResult<I, O> = nom::IResult<I, O, ProtocolParserError>;
