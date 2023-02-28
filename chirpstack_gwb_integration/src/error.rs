//! All errors for this crate.
use thiserror::Error;
use uuid::Uuid;

/// All errors this crate can return.
#[allow(missing_docs)]
#[derive(Error, Debug)]
pub enum Error {
    #[error("Downlink error: {0}")]
    Downlink(#[from] DownlinkError),
    #[error("Uuid collision, this is extremely unlikely. Try again if you encounter this error.")]
    UuidCollision,
    #[error("Config error: {0}")]
    Config(#[from] config::ConfigError),
    #[error("Reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("Runtime error: {0}")]
    Runtime(#[from] RuntimeError),
    #[error("Rumqttc error: {0}")]
    Rumqttc(#[from] rumqttc::Error),
    #[error("Rumqttc client error: {0}")]
    RumqttcClient(#[from] rumqttc::ClientError),
    #[error("Serde_json error: {0}")]
    SerdeJson(#[from] serde_json::Error),
    #[error("Hex `FromHex` conversion error error: {0}")]
    FromHex(#[from] hex::FromHexError),
    #[error("Tonic transport error: {0}")]
    TonicTransport(#[from] tonic::transport::Error),
    #[error("Tonic invalid metadata error: {0}")]
    TonicInvalidMetaData(#[from] tonic::metadata::errors::InvalidMetadataValue),
    #[error("Url invalid error: {0}")]
    TonicInvalidUri(#[from] http::uri::InvalidUri),
    #[error("gRPC error: {0}")]
    GRPCStatus(#[from] tonic::Status),
    #[error("Topic parsing error: {0}")]
    TopicParsing(#[from] TopicParsingError),
    #[error("Data rate conversion error: {0}")]
    DataRateConversion(#[from] DataRateConversionError),
}

/// Errors occuring when handling callbacks.
#[allow(missing_docs)]
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum CallbackError {
    #[error("Prost decode error: {0}")]
    ProstDecode(#[from] prost::DecodeError),
    #[error("No callback found for Uuid: {uuid}")]
    NoSuchCallback { uuid: Uuid },
}

/// Errors occurring while parsing MQTT topic strings.
#[allow(missing_docs)]
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum TopicParsingError {
    #[error("Could not parse LoRaWanRegion: {was}")]
    LoRaWanRegion { was: String },
    #[error("Could not parse TopicType: \"{was}\"")]
    TopicType { was: String },
    #[error("Could not parse CommandType: \"{was}\"")]
    CommandType { was: String },
    #[error("Could not parse StateType: \"{was}\"")]
    StateType { was: String },
    #[error("Could not parse EventType: \"{was}\"")]
    EventType { was: String },
    #[error("Topic had more than 5 elements separated by \"/\": {length}")]
    TooLong { length: usize },
    #[error("Topic had less than 5 elements separated by \"/\": {length}")]
    TooShort { length: usize },
    #[error("No \"gateway\" marker was found.")]
    NoGatewayMarker,
}

/// Errors returned by the runtime.
#[allow(missing_docs)]
#[derive(Error, Debug)]
pub enum RuntimeError {
    #[error("Already subscribed to topic: {topic}")]
    AlreadySubscribed { topic: String },
    #[error("Not subscribed to topic: {topic}")]
    NotSubscribed { topic: String },
    #[error("Gateway not found: {gateway_id}")]
    NoSuchGateway { gateway_id: String },
    #[error("Callback error: {0}")]
    Callback(#[from] CallbackError),
    #[error("Uuid collision, this is extremely unlikely. Try again if you encounter this error.")]
    UuidCollision,
    #[error("Rumqttc client error: {0}")]
    RumqttcClient(#[from] rumqttc::ClientError),
}

/// Errors occurring when creating downlinks.
#[allow(missing_docs)]
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum DownlinkError {
    #[error("Missing parameter: {missing}")]
    MissingParameter { missing: String },
    #[error("Payload is too big, over limit by: {over_limit}")]
    PayloadTooBig { over_limit: usize },
}

/// Errors occurring when converting from bandwidth and spreading factor to data rate.
#[allow(missing_docs)]
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum DataRateConversionError {
    #[error("Parameters do not match any data rate, bandwidth: {bandwidth} spreading_factor: {spreading_factor}")]
    WrongParameters {
        bandwidth: u32,
        spreading_factor: u32,
    },
}

/// Errors occurring when converting an integer to a [`SpreadingFactor`](crate::downlinks::predefined_parameters::SpreadingFactor).
#[allow(missing_docs)]
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum SpreadingFactorConversionError {
    #[error("Parameter does not match any spreading factor: {spreading_factor}")]
    NoSuchSpreadingFactor { spreading_factor: u32 },
}

/// Errors occurring when converting from integer to a [`Bandwidth`](crate::downlinks::predefined_parameters::Bandwidth).
#[allow(missing_docs)]
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum BandwidthConversionError {
    #[error("Parameter does not match any bandwidth: {bandwidth}")]
    NoSuchBandwidth { bandwidth: u32 },
}
