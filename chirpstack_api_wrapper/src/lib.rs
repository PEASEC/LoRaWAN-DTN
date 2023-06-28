//! A wrapper around parts of the ChirpStack API used by Spatz.

#![warn(missing_docs)]
#![warn(clippy::missing_errors_doc)]
#![warn(clippy::missing_panics_doc)]
#![warn(clippy::missing_docs_in_private_items)]
#![warn(clippy::pedantic)]
#![allow(clippy::doc_markdown)]

pub mod error;

use crate::error::Error;
use std::collections::HashSet;
use tracing::trace;

/// The ChirpStack API type containing information about the API endpoint and providing methods to
/// interact with the API.
#[derive(Debug)]
pub struct ChirpStackApi {
    /// Url to the ChirpStack API
    pub url: String,
    /// Port number
    pub port: u16,
    /// API token
    pub api_token: String,
    /// Tenant ID, use None used as admin
    pub tenant_id: Option<String>,
}

impl ChirpStackApi {
    /// Retrieves the available gateways from the ChirpStack API. `limit` limits the about of gateways
    /// returned by the API.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - the endpoint could not be parsed.
    /// - the endpoint could not be reached.
    /// - the bearer token could not be parsed as [`MetadataValue`](tonic::metadata::value::MetadataValue).
    /// - the list request failed.
    pub async fn request_gateways(
        &self,
        limit: u32,
    ) -> Result<chirpstack_api::api::ListGatewaysResponse, Error> {
        use tonic::{metadata::MetadataValue, transport::Channel, Request};

        trace!("Creating endpoint");
        let channel = Channel::builder(format!("{}:{}", self.url, self.port).parse()?)
            .connect_timeout(std::time::Duration::from_secs(3));

        trace!("Connecting to endpoint, creating channel");
        let channel = channel.connect().await?;

        trace!("Parsing token");
        let token: MetadataValue<_> = format!("Bearer {}", self.api_token).parse()?;

        trace!("Creating client");
        let mut client =
            chirpstack_api::api::gateway_service_client::GatewayServiceClient::with_interceptor(
                channel,
                move |mut req: Request<()>| {
                    req.metadata_mut().insert("authorization", token.clone());
                    Ok(req)
                },
            );

        trace!("Creating request");
        let request = chirpstack_api::api::ListGatewaysRequest {
            limit,
            offset: 0,
            search: String::new(),
            tenant_id: self.tenant_id.clone().unwrap_or_default(),
            multicast_group_id: String::new(),
        };
        trace!("Sending request");
        Ok(client.list(request).await?.into_inner())
    }

    /// Retrieves the available gateway IDs from the ChirpStack API. `limit` limits the about of gateways
    /// returned by the API.
    ///
    /// # Errors
    /// Returns an error if an empty gateway list was retrieved. Also returns errors on all conditions
    /// [`request_gateways`](ChirpStackApi::request_gateways) does.
    pub async fn request_gateway_ids(&self, limit: u32) -> Result<HashSet<String>, Error> {
        let mut result = HashSet::new();
        for gateway in self.request_gateways(limit).await?.result {
            result.insert(gateway.gateway_id);
        }
        if result.is_empty() {
            return Err(Error::NoGatewaysReturned);
        }
        Ok(result)
    }
}
