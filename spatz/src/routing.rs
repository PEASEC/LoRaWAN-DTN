mod flooding;

pub use flooding::flooding;

use crate::AppState;
use chirpstack_gwb_integration::downlinks::downlink_builder::DownlinkBuilder;
use chirpstack_gwb_integration::downlinks::downlink_item_builder::DownlinkItemBuilder;
use chirpstack_gwb_integration::downlinks::predefined_parameters::{DataRate, Frequency};
use chirpstack_gwb_integration::downlinks::{Downlink, DownlinkItem, ImmediatelyClassC};
use std::collections::HashSet;
use std::sync::Arc;
use tracing::error;

/// Retrieve a list of all gateways connected to the ChirpStack instance.  Returns None if an error occurred.
async fn retrieve_gateways(state: Arc<AppState>) -> Option<HashSet<String>> {
    match state.chirpstack_api.request_gateway_ids(1000).await {
        Ok(gateway_ids) => Some(gateway_ids),
        Err(e) => {
            error!(%e);
            None
        }
    }
}

/// Create a [`DownlinkItem<ImmediatelyClassC>`]. Returns None if an error occurred.
fn create_downlink_item(
    payload: Vec<u8>,
    data_rate: DataRate,
) -> Option<DownlinkItem<ImmediatelyClassC>> {
    match DownlinkItemBuilder::<ImmediatelyClassC>::new()
        .frequency(Frequency::Freq868_3)
        .data_rate(data_rate)
        .power(14)
        .phy_payload(payload)
        .board(0)
        .antenna(0)
        .build()
    {
        Ok(downlink_item) => Some(downlink_item),
        Err(e) => {
            error!(%e);
            None
        }
    }
}

/// Create a [`Downlink<ImmediatelyClassC>`]. Returns None if an error occurred.
fn create_downlink(
    gateway_id: String,
    downlink_id: u32,
    item: DownlinkItem<ImmediatelyClassC>,
) -> Option<Downlink<ImmediatelyClassC>> {
    match DownlinkBuilder::new()
        .gateway_id(gateway_id)
        .downlink_id(downlink_id)
        .add_item(item)
        .build()
    {
        Ok(downlink) => Some(downlink),
        Err(e) => {
            error!(%e);
            None
        }
    }
}
