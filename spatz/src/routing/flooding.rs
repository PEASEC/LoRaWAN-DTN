use crate::routing::{create_downlink, create_downlink_item, retrieve_gateways};
use crate::AppState;
use chirpstack_gwb_integration::downlinks::predefined_parameters::DataRate;
use rand::Rng;
use std::sync::Arc;
use tracing::{error, instrument, trace};

/// Sends the payload from every gateway connected to the ChirpStack.
#[instrument(skip_all)]
pub async fn flooding(payload: Vec<u8>, state: Arc<AppState>, data_rate: DataRate) {
    let Some(gateways) = retrieve_gateways(state.clone()).await else {
        return
    };
    let Some(downlink_item) = create_downlink_item(payload, data_rate) else {
        return
    };
    trace!("{} gateways found", gateways.len());
    for gateway in gateways {
        let Some(downlink) =  create_downlink(gateway.clone(), rand::thread_rng().gen(), downlink_item.clone()) else {
            return
        };
        trace!("Enqueuing downlink for gateway: {gateway}");
        if let Err(err) = state.runtime.enqueue(&gateway, downlink).await {
            error!(%err);
        };
    }
}
