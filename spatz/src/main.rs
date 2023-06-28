//! Daemon application to facilitate the HofBox communication over LoRaWAN.

#![deny(clippy::unwrap_used)]
#![warn(missing_docs)]
#![warn(clippy::missing_errors_doc)]
#![warn(clippy::missing_panics_doc)]
#![warn(clippy::missing_docs_in_private_items)]
#![warn(clippy::pedantic)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::module_name_repetitions)]

mod api;
mod app_start;
mod bundle_processing;
mod configuration;
mod database;
mod duty_cycle_manager;
mod end_device_id;
mod error;
mod gateway_ids_manager;
mod graceful_shutdown;
mod lora_modulation_extraction;
mod lorawan_protocol;
mod packet_cache;
mod packet_queue_manager;
mod receive_buffers;
mod routing;
mod send_buffers;
mod uplink_processing;

use crate::app_start::start_app;
use crate::configuration::Configuration;
use crate::database::save_state_to_db;
use crate::duty_cycle_manager::DutyCycleManager;
use crate::end_device_id::ManagedEndDeviceId;
use crate::gateway_ids_manager::GatewayIdsManager;
use crate::graceful_shutdown::{ShutdownConditions, ShutdownGenerator, ShutdownInitiator};
use crate::packet_queue_manager::QueueManager;
use crate::routing::RoutingAlgorithm;
use chirpstack_api_wrapper::ChirpStackApi;
use chrono::Duration;
use packet_cache::PacketCache;
use sqlx::SqlitePool;
use std::collections::HashSet;
use std::panic::PanicInfo;
use std::sync::Arc;
use tokio::signal;
use tokio::sync::{broadcast, mpsc, Mutex};
use tracing::{error, trace};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Layer};

/// Configuration management of the Spatz instance.
pub struct SpatzConfig {
    /// Configuration of the Spatz instance. May not reflect the config of the running
    /// instance but the config applied at the next start.
    pub next_configuration: Configuration,
    /// Current configuration of the Spatz instance.
    pub currently_active_configuration: Configuration,
}

impl SpatzConfig {
    /// Returns whether configurations have been changed which require a restart.
    #[must_use]
    pub fn restart_pending(&self) -> bool {
        self.next_configuration != self.currently_active_configuration
    }
}

/// State of the daemon application.
pub struct AppState {
    /// Channel from the websocket handler to the bundle handler task.
    pub bundles_from_ws: mpsc::Sender<bp7::Bundle>,
    /// Channel to the websocket handler for received bundles.
    pub bundles_to_ws: broadcast::Sender<bp7::Bundle>,
    /// The chirpstack_gwb_integration runtime.
    pub runtime: chirpstack_gwb_integration::runtime::Runtime,
    /// The end device IDs used in the daemon.
    pub end_device_ids: Arc<Mutex<HashSet<ManagedEndDeviceId>>>,
    /// ChirpStack API information.
    pub chirpstack_api: ChirpStackApi,
    /// Cache to keep track of recently received packets.
    pub packet_cache: PacketCache,
    /// Duty cycle manager.
    pub duty_cycle_manager: Arc<Mutex<DutyCycleManager>>,
    /// Packet and buffer queue manager.
    pub queue_manager: Arc<QueueManager>,
    /// Gateway IDs connected to this spatz.
    pub gateway_ids_manager: GatewayIdsManager,
    /// The current routing algorithm.
    pub routing_algo: Box<dyn RoutingAlgorithm>,
    /// Connection pool to the Sqlite DB.
    pub db_pool: SqlitePool,
    /// Restart initiator.
    pub restart_initiator: ShutdownInitiator,
    /// Configuration management.
    pub configuration: Arc<Mutex<SpatzConfig>>,
}

#[tokio::main]
async fn main() {
    #[cfg(debug_assertions)]
    let filter_directives =
        std::env::var("RUST_LOG").unwrap_or_else(|_| "spatz=trace,tower_http=trace".into());

    #[cfg(not(debug_assertions))]
    let filter_directives = std::env::var("RUST_LOG").unwrap_or_else(|_| "spatz=error".into());

    tracing_subscriber::registry()
        // RUST_LOG=spatz::uplink_processing
        .with(
            tracing_subscriber::fmt::layer()
                .with_filter(tracing_subscriber::EnvFilter::new(filter_directives)),
        )
        .init();

    loop {
        // repeatable to restart systems
        let graceful_shutdown_generator = ShutdownGenerator::new();
        let shutdown_agent = graceful_shutdown_generator.generate_agent();
        let panic_shutdown_initiator = graceful_shutdown_generator.generate_initiator();
        let app_shutdown_initiator = graceful_shutdown_generator.generate_initiator();
        // Remove earlier custom panic hook.
        let _ = std::panic::take_hook();
        let default_panic = std::panic::take_hook();
        set_panic_hook(default_panic, panic_shutdown_initiator);
        let Ok(state) = start_app(shutdown_agent, app_shutdown_initiator).await else {
            return;
        };

        let mut shutdown_control = graceful_shutdown_generator.generate_control();

        tokio::select! {
            ctrl_c_result = signal::ctrl_c()  => {
                match ctrl_c_result {
                    Ok(()) => {
                        // do graceful shutdown routine
                        trace!("Graceful shutdown initiated");
                        shutdown_control.start_shutdown();
                        shutdown_control.await_complete_shutdown(15).await;
                        save_state_to_db(state).await;
                        return;
                    }
                    Err(err) => {
                        error!("Failed to listen for shutdown event: {}", err);
                        return;
                    }
                }
            },
            shutdown_initiation = shutdown_control.await_shutdown_initiation() => {
                if let Some(shutdown_initiation) = shutdown_initiation {
                    match shutdown_initiation {
                        ShutdownConditions::Panic => {
                            trace!("Some task panicked, shutting down");
                            shutdown_control.start_shutdown();
                            shutdown_control.await_complete_shutdown(15).await;
                        },
                        ShutdownConditions::MqttError => {
                            trace!("MQTT connection error, shutting down");
                            shutdown_control.start_shutdown();
                            shutdown_control.await_complete_shutdown(15).await;
                        }
                        ShutdownConditions::GatewayRetrievalFailed => {
                            trace!("Failed to retrieve gateways, shutting down");
                            shutdown_control.start_shutdown();
                            shutdown_control.await_complete_shutdown(15).await;
                        }
                        ShutdownConditions::AxumStartFailed => {
                            trace!("Failed to start axum server, shutting down");
                            shutdown_control.start_shutdown();
                            shutdown_control.await_complete_shutdown(15).await;
                        }
                        ShutdownConditions::Restart => {
                            trace!("Restarting all Spatz");
                            shutdown_control.start_shutdown();
                            shutdown_control.await_complete_shutdown(15).await;
                            save_state_to_db(state).await;
                            continue;
                        }
                    }
                    save_state_to_db(state).await;
                } else {
                    trace!("No more shutdown agents, shutting down");
                }
                return;
            }
        }
    }
}

/// Sets the panic hook.
///
/// Integrates the graceful shutdown mechanism into panics.
fn set_panic_hook(
    default_panic: Box<dyn Fn(&PanicInfo) + Send + Sync>,
    shutdown_initiator: ShutdownInitiator,
) {
    std::panic::set_hook(Box::new(move |panic_info| {
        default_panic(panic_info);
        shutdown_initiator.initiate_shutdown(ShutdownConditions::Panic);
    }));
}
