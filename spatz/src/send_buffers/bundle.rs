//! Bundle send buffer.

use crate::end_device_id::EndDeviceId;
use crate::error::{
    BundleSendBufferConversionError, BundleSendBufferCreationError, SendBufferError,
};
use crate::lorawan_protocol::{
    BundleFragment, CompleteBundle, LoRaWanPacket, BUNDLE_FRAGMENT_HEADERS_SIZE,
    COMPLETE_BUNDLE_HEADERS_SIZE,
};
use crate::send_buffers::SendBuffer;
use bp7::dtntime::DtnTimeHelpers;
use bp7::Bundle;
use chirpstack_gwb_integration::downlinks::predefined_parameters::DataRate;
use chrono::{DateTime, NaiveDateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Send buffer for bundles.
///
/// Splits one bundle into multiple packets to be sent over LoRaWAN.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BundleSendBuffer {
    /// Destination.
    destination: EndDeviceId,
    /// End device ID.
    source: EndDeviceId,
    /// Timestamp.
    timestamp: DateTime<Utc>,
    /// The fragment index of the packet to be sent next.
    fragment_index: u8,
    /// The payload, will be fragmented and sent via multiple packets.
    payload: Vec<u8>,
}

impl BundleSendBuffer {
    /// Creates a new [`BundleSendBuffer`].
    ///
    /// # Errors
    ///
    /// Returns an error if the payload is too large and cannot be sent completely at the lowest data rate.
    pub fn new(
        destination: EndDeviceId,
        source: EndDeviceId,
        timestamp: DateTime<Utc>,
        payload: Vec<u8>,
    ) -> Result<Self, BundleSendBufferCreationError> {
        if payload.len()
            > (DataRate::Eu863_870Dr0.max_usable_payload_size(false) - BUNDLE_FRAGMENT_HEADERS_SIZE)
                * 128
        {
            Err(BundleSendBufferCreationError::PayloadTooLarge)
        } else {
            Ok(Self {
                destination,
                source,
                timestamp,
                fragment_index: 0,
                payload,
            })
        }
    }
}

impl SendBuffer for BundleSendBuffer {
    fn next_packet(
        &mut self,
        data_rate: DataRate,
    ) -> Result<Box<dyn LoRaWanPacket>, SendBufferError> {
        if self.payload.is_empty() {
            return Err(SendBufferError::PayloadConsumed);
        }
        let packet_max_size =
            data_rate.max_usable_payload_size(false) - COMPLETE_BUNDLE_HEADERS_SIZE;
        if self.fragment_index == 0 && self.payload.len() <= packet_max_size {
            let complete_bundle = CompleteBundle::new(
                self.destination,
                self.source,
                self.timestamp,
                &mut self.payload,
                data_rate,
            )
            .expect("Payload size checking is wrong");
            Ok(Box::new(complete_bundle))
        } else if packet_max_size <= self.payload.len() {
            let bundle_fragment = BundleFragment::new(
                self.destination,
                self.source,
                self.timestamp,
                true,
                self.fragment_index,
                &mut self.payload,
                data_rate,
            )
            .expect("Payload size checking is wrong");
            self.fragment_index += 1;
            Ok(Box::new(bundle_fragment))
        } else {
            let bundle_fragment = BundleFragment::new(
                self.destination,
                self.source,
                self.timestamp,
                false,
                self.fragment_index,
                &mut self.payload,
                data_rate,
            )
            .expect("Payload size checking is wrong");
            self.fragment_index += 1;
            Ok(Box::new(bundle_fragment))
        }
    }

    fn is_empty(&self) -> bool {
        self.payload.is_empty()
    }
}

impl TryFrom<Bundle> for BundleSendBuffer {
    type Error = BundleSendBufferConversionError;

    fn try_from(bundle: Bundle) -> Result<Self, Self::Error> {
        let payload = if let Some(payload) = bundle.payload() {
            payload.clone()
        } else {
            return Err(BundleSendBufferConversionError::NoPayload);
        };
        let primary = bundle.primary;
        let source: EndDeviceId = primary.source.try_into()?;
        let destination: EndDeviceId = primary.destination.try_into()?;
        let Some(naive_time) =
            NaiveDateTime::from_timestamp_opt(
                i64::try_from(
                    primary.creation_timestamp.dtntime().unix()).expect("Dtn time does not fit into i64"), 0) else {
            return Err(BundleSendBufferConversionError::TryFromTimestampError);
        };
        let timestamp = DateTime::from_utc(naive_time, Utc);
        Ok(BundleSendBuffer::new(
            destination,
            source,
            timestamp,
            payload,
        )?)
    }
}
