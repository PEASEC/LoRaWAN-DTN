//! Builders to create correct downlinks.

use crate::downlinks::{Downlink, DownlinkItem, DownlinkType};
use crate::error::DownlinkBuilderError;

/// Builder for [`Downlink`].
#[derive(Debug, Clone)]
pub struct DownlinkBuilder<Dt>
where
    Dt: DownlinkType,
{
    /// Gateway ID.
    gateway_id: Option<String>,
    /// In the ChirpStack source, this is set by `rand::thread_rng().gen()`.
    downlink_id: Option<u32>,
    /// Downlink items.
    items: Option<Vec<DownlinkItem<Dt>>>,
}

impl<Dt> Default for DownlinkBuilder<Dt>
where
    Dt: DownlinkType,
{
    fn default() -> Self {
        Self {
            gateway_id: None,
            downlink_id: None,
            items: None,
        }
    }
}

impl<Dt> DownlinkBuilder<Dt>
where
    Dt: DownlinkType,
{
    /// Creates a new [`DownlinkBuilder`].
    #[must_use]
    pub fn new() -> Self {
        DownlinkBuilder::default()
    }

    /// Sets the gateway id
    pub fn gateway_id(&mut self, gateway_id: String) -> &mut Self {
        self.gateway_id = Some(gateway_id);
        self
    }

    /// Sets downlink id ([`uuid`]).
    pub fn downlink_id(&mut self, downlink_id: u32) -> &mut Self {
        self.downlink_id = Some(downlink_id);
        self
    }

    /// Adds a single item to the items list.
    pub fn add_item(&mut self, item: DownlinkItem<Dt>) -> &mut Self {
        if self.items.is_none() {
            self.items = Some(Vec::with_capacity(1));
        }
        self.items
            .as_mut()
            .expect("This can't happen, variable is checked to be Some(_) before")
            .push(item);
        self
    }

    /// Adds multiple items to the item list.
    pub fn add_items(&mut self, mut items: Vec<DownlinkItem<Dt>>) -> &mut Self {
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
    ///
    /// # Errors
    ///
    /// Returns an error if a required parameter is missing.
    pub fn build(&mut self) -> Result<Downlink<Dt>, DownlinkBuilderError> {
        if self.items.is_none() {
            return Err(DownlinkBuilderError::MissingParameter {
                missing: "items".to_owned(),
            });
        }
        if self.downlink_id.is_none() {
            return Err(DownlinkBuilderError::MissingParameter {
                missing: "downlink_id".to_owned(),
            });
        }
        if self.gateway_id.is_none() {
            return Err(DownlinkBuilderError::MissingParameter {
                missing: "gateway_id".to_owned(),
            });
        }

        Ok(Downlink {
            gateway_id: self
                .gateway_id
                .clone()
                .expect("This can't happen, variable is checked for None before."),
            downlink_id: self
                .downlink_id
                .expect("This can't happen, variable is checked for None before."),
            items: self
                .items
                .clone()
                .expect("This can't happen, variable is checked for None before."),
        })
    }
}
