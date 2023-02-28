mod error;

use crate::error::Error;
use std::collections::HashSet;

/// Struct to interact with the ChirpStack API. Only allows retrieving gateway IDs or info about
/// gateways.
#[derive(Debug)]
pub struct ChirpStackApi {
    pub url: String,
    pub port: u16,
    pub api_token: String,
    pub tenant_id: Option<String>,
}

impl ChirpStackApi {
    /// Retrieve the available gateways from the ChirpStack API. `limit` limits the about of gateways
    /// returned by the API.
    pub async fn request_gateways(
        &self,
        limit: u32,
    ) -> Result<chirpstack_api::api::ListGatewaysResponse, Error> {
        use tonic::{metadata::MetadataValue, transport::Channel, Request};

        let channel = Channel::builder(format!("{}:{}", self.url, self.port).parse()?)
            .connect()
            .await?;

        let token: MetadataValue<_> = format!("Bearer {}", self.api_token).parse()?;
        let mut client =
            chirpstack_api::api::gateway_service_client::GatewayServiceClient::with_interceptor(
                channel,
                move |mut req: Request<()>| {
                    req.metadata_mut().insert("authorization", token.clone());
                    Ok(req)
                },
            );

        let request = chirpstack_api::api::ListGatewaysRequest {
            limit,
            offset: 0,
            search: "".to_owned(),
            tenant_id: self.tenant_id.clone().unwrap_or_default(),
        };
        Ok(client.list(request).await?.into_inner())
    }

    /// Retrieve the available gateway IDs from the ChirpStack API. `limit` limits the about of gateways
    /// returned by the API.
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
