//! Gateway IDs manager keeps the gateway IDs of all connected gateways up to date.

use crate::graceful_shutdown::{ShutdownAgent, ShutdownConditions};
use crate::AppState;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, instrument, trace};

/// Manages all gateway IDs connected to this spatz.
#[derive(Debug)]
pub struct GatewayIdsManager {
    /// Hashset of all gateway IDs.
    pub gateway_ids: Arc<Mutex<HashSet<String>>>,
    /// The interval between updates.
    update_interval: std::time::Duration,
}
impl GatewayIdsManager {
    /// Creates a new [`GatewayIdsManager`] with the provided update interval.
    pub fn new(update_interval: std::time::Duration) -> Self {
        Self {
            gateway_ids: Arc::new(Mutex::new(HashSet::new())),
            update_interval,
        }
    }

    /// Update list of gateways connected to this spatz.
    #[instrument(skip_all)]
    pub async fn update_gateways(&self, state: Arc<AppState>, mut shutdown_agent: ShutdownAgent) {
        trace!("Starting up");
        let mut retry = 0;
        loop {
            trace!("Requesting gateways");
            if retry <= 3 {
                tokio::select! {
                    res = state.chirpstack_api.request_gateway_ids(1000) => {
                        match res {
                            Ok(gateway_ids) => {
                                *self.gateway_ids.lock().await = gateway_ids;
                                retry = 0;
                            }
                            Err(err) => {
                                error!(%err);
                                retry += 1;
                                continue;
                            }
                        }
                    },
                    _ = shutdown_agent.await_shutdown() => {
                        trace!("Shutting down");
                        return
                    }
                }
            } else {
                error!("Failed to retrieve gateways after three tries");
                shutdown_agent.initiate_shutdown(ShutdownConditions::GatewayRetrievalFailed);
            }

            tokio::select! {
                _ = tokio::time::sleep(self.update_interval) => {},
                _ = shutdown_agent.await_shutdown() => {
                    trace!("Shutting down");
                    return
                }
            }
        }
    }
}
