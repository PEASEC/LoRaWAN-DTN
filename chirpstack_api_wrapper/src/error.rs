//! All errors for this crate.

use thiserror::Error;

/// All errors this crate can return.
#[allow(clippy::missing_docs_in_private_items)]
#[derive(Error, Debug)]
pub enum Error {
    /// Tonic transport error.
    #[error("Tonic transport error: {0}")]
    TonicTransport(#[from] tonic::transport::Error),
    /// Tonic invalid metadata error.
    #[error("Tonic invalid metadata error: {0}")]
    TonicInvalidMetaData(#[from] tonic::metadata::errors::InvalidMetadataValue),
    /// gRPC error.
    #[error("gRPC error: {0}")]
    GRPCStatus(#[from] tonic::Status),
    /// Url invalid error.
    #[error("Url invalid error: {0}")]
    TonicInvalidUri(#[from] http::uri::InvalidUri),
    /// No gateway IDs returned by ChirpStack API.
    #[error("No gateway IDs returned by ChirpStack API")]
    NoGatewaysReturned,
}
