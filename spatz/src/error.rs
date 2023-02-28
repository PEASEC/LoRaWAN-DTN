use chirpstack_gwb_integration::error::{BandwidthConversionError, SpreadingFactorConversionError};
use nom::error::{FromExternalError, ParseError};
use nom::ErrorConvert;
use std::num::{ParseIntError, TryFromIntError};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Message cache error: {0}")]
    MessageCache(#[from] MessageCacheError),
    #[error("Protocol parser error: {0}")]
    ProtocolParser(#[from] ProtocolParserError),
    #[error("Protocol Creation error: {0}")]
    ProtocolCreation(#[from] ProtocolCreationError),
    #[error("Location encoding error: {0}")]
    LocationEncoding(#[from] LocationEncodingError),
    #[error("Send buffer error: {0}")]
    SendBuffer(#[from] SendBufferError),
    #[error("Receive buffer error: {0}")]
    ReceiveBuffer(#[from] ReceiveBufferError),
    #[error("Try from endpoint ID error: {0}")]
    TryFromEndpointId(#[from] TryFromEndpointIdError),
    #[error("Bp7 endpoint ID error: {0}")]
    Bp7Eid(#[from] bp7::eid::EndpointIdError),
}

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum MessageCacheError {
    #[error("Entry has not timed out yet")]
    NotTimedOut,
}

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum ProtocolParserError {
    #[error("Payload has no proprietary tag.")]
    NoProprietaryTag,
    #[error("Payload has wrong version tag.")]
    WrongVersionTag,
    #[error("Payload has unknown message type.")]
    UnknownMsgType,
    #[error("The index of the fragment is bigger than the total number of fragments")]
    FragmentIndexBiggerThanTotal,
    #[error("Nom encountered an error: {0:?}")]
    Nom(nom::error::ErrorKind),
    #[error("Did not receive three bytes, cannot convert to u32")]
    NotThreeBytes,
    #[error("Failed to create naive datetime from timestamp")]
    FromTimestampError,
}

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum ProtocolCreationError {
    #[error("Fragment index was larger than (2^7)-1")]
    FragmentIndexTooLarge,
    #[error("Fragment total amount was larger than (2^7)")]
    FragmentTotalAmountTooLarge,
}

#[allow(clippy::enum_variant_names)]
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum LocationEncodingError {
    #[error("Could not encode value, was out of range (-90.00 to 90.00)")]
    LatOutOfRange,
    #[error("Could not encode value, was out of range (-180.00 to 180.00)")]
    LongOutOfRange,
    #[error("Could not encode value, was out of range (-83886.00 to 83886.00)")]
    AltOutOfRange,
}

#[derive(Error, Debug)]
pub enum SendBufferError {
    #[error("There are not enough bytes per fragment to send this packet")]
    NotEnoughBytesPerFragment,
    #[error("There are not enough bytes per fragment to send at least one payload part")]
    NotEnoughBytesPerFragmentForOnePayload,
    #[error("All already fragments retrieved")]
    NoRemainingFragments,
    #[error("Too many canonical blocks, only one is supported")]
    TooManyCanonicals,
    #[error("The canonical block contains no data")]
    NoDataCanonical,
    #[error("The payload is too big and cannot be sent with selected data rate")]
    PayloadTooBig,
    #[error("Endpoint conversion error: ")]
    EndpointConversion(#[from] TryFromEndpointIdError),
    #[error("Protocol creation error: ")]
    ProtocolCreation(#[from] ProtocolCreationError),
    #[error("Payload was empty")]
    EmptyPayload,
    #[error("Fragment count calculation is wrong, payload returned None")]
    FragmentCountCalculationWrong,
    #[error("Failed to create naive datetime from timestamp")]
    FromTimestampError,
    #[error("Message cache error: ")]
    MessageCacheError(#[from] MessageCacheError),
}

#[allow(missing_docs)]
#[derive(Error, Debug, PartialEq, Eq)]
pub enum AirtimeCalculationError {
    #[error("No downlink items")]
    NoItems,
    #[error("Data extraction error: {0}")]
    DataRateExtraction(#[from] LoRaModulationExtrationError),
    #[error("Failed to convert integer: {0}")]
    NumberConversion(#[from] TryFromIntError),
    #[error("Failed to convert bandwidth: {0}")]
    BandWidthConversion(#[from] BandwidthConversionError),
    #[error("Failed to convert spreading factor: {0}")]
    SpreadingFactorConversion(#[from] SpreadingFactorConversionError),
}

#[derive(Error, Debug)]
pub enum SendManagerError {
    #[error("SendBuffer does not contain any more fragments")]
    NoRemainingFragments,
    #[error("No SendBuffer in SendBuffer queue")]
    NoSendBufferInQueue,
    #[error(transparent)]
    SendBufferError(#[from] SendBufferError),
}

#[derive(Error, Debug)]
pub enum SubBandError {
    #[error("No matching sub band for frequency: {freq}")]
    NoMatchingSubBand { freq: u32 },
}

#[derive(Error, Debug)]
pub enum DutyCycleManagerError {
    #[error("More capacity used than was available")]
    CapacityOverused ,
    #[error(transparent)]
    SubBand(#[from] SubBandError),
}

#[derive(Error, Debug)]
pub enum TryFromEndpointIdError {
    #[error("Only Dtn addressing is supported")]
    NoDtnAddress,
    #[error("Error parsing int: {0}")]
    ParseInt(#[from] ParseIntError),
    #[error("Endpoint id error from bp7: {0}")]
    EndpointId(#[from] bp7::eid::EndpointIdError),
    #[error("Primary builder error from bp7: {0}")]
    PrimaryBuilder(#[from] bp7::primary::PrimaryBuilderError),
}

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum ReceiveBufferError {
    #[error("A frame with this index has already been received")]
    IndexAlreadyReceived,
    #[error("Payload was not a fragment")]
    NoFragment,
    #[error("Not all fragments have been received")]
    FragmentsMissing,
    #[error(
        "The total fragment amount of the fragment does not match previously received fragments"
    )]
    FragmentAmountDoesNotMatch,
}

#[allow(missing_docs)]
#[derive(Error, Debug, PartialEq, Eq)]
pub enum LoRaModulationExtrationError {
    #[error("Wrong parameters: {0}")]
    WrongParameters(#[from] chirpstack_gwb_integration::error::DataRateConversionError),
    #[error("No TX info in uplink frame")]
    NoTxInfo,
    #[error("No modulation info in uplink frame")]
    NoModulationInfo,
    #[error("No LoRa parameters in modulation in uplink frame")]
    NoLoRaParameters,
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

pub type IResult<I, O> = nom::IResult<I, O, ProtocolParserError>;
