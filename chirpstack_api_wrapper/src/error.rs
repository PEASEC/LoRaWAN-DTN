use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Tonic transport error: {0}")]
    TonicTransport(#[from] tonic::transport::Error),
    #[error("Tonic invalid metadata error: {0}")]
    TonicInvalidMetaData(#[from] tonic::metadata::errors::InvalidMetadataValue),
    #[error("gRPC error: {0}")]
    GRPCStatus(#[from] tonic::Status),
    #[error("Url invalid error: {0}")]
    TonicInvalidUri(#[from] http::uri::InvalidUri),
    #[error("No gateway IDs returned by ChirpStack API")]
    NoGatewaysReturned,
}
