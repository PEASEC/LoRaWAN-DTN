//! Hook into ChirpStack Gateway Bridge MQTT communication.
//!
//! A library to facilitate hooking into the ChirpStack gateway bridge.
//! Allows adding callbacks to incoming MQTT messages per gateway and type or for all gateways per
//! type.

#![warn(missing_docs)]

pub mod downlinks;
pub mod error;
pub mod gateway_topics;
pub mod runtime;
