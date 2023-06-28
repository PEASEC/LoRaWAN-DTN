//! Hook into ChirpStack Gateway Bridge MQTT communication.
//!
//! A library to facilitate hooking into the ChirpStack gateway bridge.
//! Allows adding callbacks to incoming MQTT messages per gateway and type or for all gateways per
//! type.

#![warn(missing_docs)]
#![warn(clippy::missing_errors_doc)]
#![warn(clippy::missing_panics_doc)]
#![warn(clippy::missing_docs_in_private_items)]
#![warn(clippy::pedantic)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::module_name_repetitions)]

pub mod downlinks;
pub mod error;
pub mod gateway_topics;
pub mod runtime;
