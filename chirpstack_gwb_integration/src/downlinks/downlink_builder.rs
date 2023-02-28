//! Builders to create correct downlinks.

use crate::downlinks::{Downlink, DownlinkItem};
use crate::error::DownlinkError;

/// Populate with data and build a [`Downlink`].
#[derive(Debug, Clone)]
pub struct DownlinkBuilder<DownlinkType> {
    gateway_id: Option<String>,
    /// In the ChirpStack source, this is set by `rand::thread_rng().gen()`.
    downlink_id: Option<u32>,
    items: Option<Vec<DownlinkItem<DownlinkType>>>,
}

impl<DownlinkType> Default for DownlinkBuilder<DownlinkType> {
    fn default() -> Self {
        Self {
            gateway_id: None,
            downlink_id: None,
            items: None,
        }
    }
}

impl<DownlinkType> DownlinkBuilder<DownlinkType> {
    /// Create a new [`DownlinkBuilder`].
    pub fn new() -> Self {
        DownlinkBuilder::default()
    }

    /// Set the gateway id
    pub fn gateway_id(mut self, gateway_id: String) -> DownlinkBuilder<DownlinkType> {
        self.gateway_id = Some(gateway_id);
        self
    }

    /// Set downlink id ([`uuid`]).
    pub fn downlink_id(mut self, downlink_id: u32) -> DownlinkBuilder<DownlinkType> {
        self.downlink_id = Some(downlink_id);
        self
    }

    /// Add a single item to the items list.
    pub fn add_item(mut self, item: DownlinkItem<DownlinkType>) -> DownlinkBuilder<DownlinkType> {
        if self.items.is_none() {
            self.items = Some(Vec::with_capacity(1));
        }
        self.items
            .as_mut()
            .expect("This can't happen, variable is checked to be Some(_) before")
            .push(item);
        self
    }

    /// Add multiple items to the item list.
    pub fn add_items(
        mut self,
        mut items: Vec<DownlinkItem<DownlinkType>>,
    ) -> DownlinkBuilder<DownlinkType> {
        if self.items.is_none() {
            self.items = Some(Vec::with_capacity(items.len()));
        }
        self.items
            .as_mut()
            .expect("This can't happen, variable is checked to be Some(_) before")
            .append(&mut items);
        self
    }

    /// Builds the [`Downlink`].
    pub fn build(self) -> Result<Downlink<DownlinkType>, DownlinkError> {
        if self.items.is_none() {
            return Err(DownlinkError::MissingParameter {
                missing: "items".to_owned(),
            });
        }
        if self.downlink_id.is_none() {
            return Err(DownlinkError::MissingParameter {
                missing: "downlink_id".to_owned(),
            });
        }
        if self.gateway_id.is_none() {
            return Err(DownlinkError::MissingParameter {
                missing: "gateway_id".to_owned(),
            });
        }

        Ok(Downlink {
            gateway_id: self
                .gateway_id
                .expect("This can't happen, variable is checked for None before."),
            downlink_id: self
                .downlink_id
                .expect("This can't happen, variable is checked for None before."),
            items: self
                .items
                .expect("This can't happen, variable is checked for None before."),
        })
    }
}
