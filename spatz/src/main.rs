//! Daemon application to facilitate the HofBox communication over LoRaWAN.

#![deny(clippy::unwrap_used)]
#![warn(missing_docs)]

mod api;
mod bundle_processing;
mod configuration;
mod duty_cycle_manager;
mod end_device_id;
mod error;
mod lora_modulation_extraction;
mod lorawan_protocol;
mod message_cache;
mod receive_buffers;
mod routing;
mod send_buffers;
mod send_manager;
mod uplink_processing;

use crate::bundle_processing::bundles_processor_task;
use crate::configuration::{CliParameters, Configuration};
use crate::duty_cycle_manager::{DownlinkCallback, DutyCycleManager};
use crate::end_device_id::ManagedEndDeviceId;
use crate::send_manager::SendManager;
use ::config::Config;
use aide::axum::ApiRouter;
use aide::openapi::{Info, OpenApi};
use aide::redoc::Redoc;
use axum::Extension;
use chirpstack_api_wrapper::ChirpStackApi;
use chrono::Duration;
use clap::Parser;
use end_device_id::EndDeviceId;
use message_cache::MessageCache;
use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tower_http::cors::CorsLayer;
use tower_http::trace::{DefaultMakeSpan, TraceLayer};
use tracing::{error, trace};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Layer};
use uplink_processing::UplinkCallback;

/// State of the daemon application.
#[derive(Debug)]
pub struct AppState {
    /// Channel from the websocket handler to the bundle handler task.
    pub bundles_from_ws: tokio::sync::mpsc::Sender<bp7::Bundle>,
    /// Channel to the websocket handler for received bundles.
    pub bundles_to_ws: tokio::sync::broadcast::Sender<bp7::Bundle>,
    /// The chirpstack_gwb_integration runtime.
    pub runtime: chirpstack_gwb_integration::runtime::Runtime,
    /// The end device IDs used in the daemon.
    pub end_device_ids: Arc<Mutex<HashSet<ManagedEndDeviceId>>>,
    /// ChirpStack API information.
    pub chirpstack_api: ChirpStackApi,
    /// Cache to keep track of recently received messages.
    pub message_cache: MessageCache,
    /// Duty cycle manager
    pub duty_cycle_manager: Arc<Mutex<DutyCycleManager>>,
}

#[tokio::main]
async fn main() {
    #[cfg(debug_assertions)]
    let filter_directives =
        std::env::var("RUST_LOG").unwrap_or_else(|_| "spatz=trace,tower_http=trace".into());

    #[cfg(not(debug_assertions))]
    let filter_directives = std::env::var("RUST_LOG").unwrap_or_else(|_| "spatz=error".into());

    tracing_subscriber::registry()
        // RUST_LOG=daemon::RUST_LOG=spatz::uplink_processing
        .with(
            tracing_subscriber::fmt::layer()
                .with_filter(tracing_subscriber::EnvFilter::new(filter_directives)),
        )
        .init();

    trace!("Parsing cli parameters");
    let cli_parameters = CliParameters::parse();

    trace!("Building configuration");
    let configuration = Config::builder()
        .add_source(config::File::with_name(&cli_parameters.config_file_path))
        .build()
        .expect("Failed to read config");
    trace!("Deserializing configuration");
    let configuration: Configuration = configuration
        .try_deserialize::<Configuration>()
        .expect("Failed to deserialize config");

    trace!("Creating channels");
    let (bundles_from_ws_sender, bundles_from_ws_receiver) = tokio::sync::mpsc::channel(10);
    let (bundles_to_ws_sender, _) = tokio::sync::broadcast::channel(10);
    let (uplink_callback_sender, uplink_callback_receiver) = tokio::sync::mpsc::channel(10);
    let (relay_sender, relay_receiver) = tokio::sync::mpsc::channel(10);
    let (bundle_send_buffer_sender, bundle_send_buffer_receiver) = tokio::sync::mpsc::channel(10);
    let (_announcement_send_buffer_sender, announcement_send_buffer_receiver) =
        tokio::sync::mpsc::channel(10);
    let (downlink_callback_sender, downlink_callback_receiver) = tokio::sync::mpsc::channel(10);

    trace!("Creating runtime");
    let mut runtime = match chirpstack_gwb_integration::runtime::Runtime::new(
        &configuration.mqtt.client_id,
        &configuration.mqtt.url,
        configuration.mqtt.port,
    )
    .await
    {
        Ok(runtime) => runtime,
        Err(e) => {
            error!("Failed to create runtime: {e}");
            return;
        }
    };

    trace!("Creating ChirpStack API info");
    let chirpstack_api = ChirpStackApi {
        url: configuration.chirpstack_api.url,
        port: configuration.chirpstack_api.port,
        api_token: configuration.chirpstack_api.api_token,
        tenant_id: configuration.chirpstack_api.tenant_id,
    };

    trace!("Creating message cache");
    let message_cache = MessageCache::new(
        configuration.daemon.message_cache.timeout_minutes,
        configuration.daemon.message_cache.cleanup_interval_seconds,
        configuration.daemon.message_cache.reset_timeout,
    );

    trace!("Calculating end device IDs");
    let end_device_ids: HashSet<ManagedEndDeviceId> = configuration
        .daemon
        .end_device_ids
        .iter()
        .map(ManagedEndDeviceId::from) //|end_device_id_string| end_device_id_string.into())
        .collect();

    trace!("Creating duty cycle manager");
    let duty_cycle_manager = Arc::new(Mutex::new(DutyCycleManager::new()));

    trace!("Creating state");
    let state = Arc::new(AppState {
        bundles_to_ws: bundles_to_ws_sender,
        bundles_from_ws: bundles_from_ws_sender,
        runtime: runtime.clone(),
        end_device_ids: Arc::new(Mutex::new(end_device_ids)),
        chirpstack_api,
        message_cache,
        duty_cycle_manager,
    });
    trace!("Creating send manager");
    let send_manager = Arc::new(SendManager::new(
        configuration.daemon.send_config.relay_queue_size,
        configuration.daemon.send_config.bundle_queue_size,
        configuration.daemon.send_config.announcement_queue_size,
        std::time::Duration::from_secs(configuration.daemon.send_config.periodic_send_delay),
    ));

    trace!("Spawning send_manager::consolidate_send_items task");
    let send_manager_clone = send_manager.clone();
    tokio::spawn(async move {
        send_manager_clone
            .consolidate_send_items_task(
                relay_receiver,
                bundle_send_buffer_receiver,
                announcement_send_buffer_receiver,
            )
            .await;
    });
    trace!("Spawning send_manager::periodic_send task");
    let state_clone = state.clone();
    tokio::spawn(async move {
        send_manager.periodic_send_task(state_clone).await;
    });

    trace!("Spawning message cache clean task");
    let state_clone = state.clone();
    tokio::spawn(async move { message_cache::cache_clean_task(state_clone).await });

    trace!("Spawning duty cycle reset task");
    let state_clone = state.clone();
    tokio::spawn(async move { duty_cycle_manager::duty_cycle_reset_task(state_clone).await });

    trace!("Spawning duty cycle manager callback task");
    let state_clone = state.clone();
    tokio::spawn(async move {
        duty_cycle_manager::downlink_duty_cycle_collector_task(
            downlink_callback_receiver,
            state_clone,
        )
        .await
    });

    trace!("Spawning uplink processor task");
    let state_clone = state.clone();
    tokio::spawn(async move {
        uplink_processing::uplink_processor_task(
            uplink_callback_receiver,
            relay_sender,
            state_clone,
        )
        .await;
    });

    trace!("Spawning bundles processor task");
    tokio::spawn(async move {
        bundles_processor_task(bundles_from_ws_receiver, bundle_send_buffer_sender).await;
    });

    trace!("Adding universal uplink callback to runtime");
    if let Err(e) = runtime
        .add_event_up_callback(
            None,
            Box::new(UplinkCallback {
                sender: uplink_callback_sender,
            }),
        )
        .await
    {
        error!("Failed to add callback to mqtt runtime: {e}");
        return;
    }

    trace!("Adding universal downlink callback to runtime");
    if let Err(e) = runtime
        .add_command_down_callback(
            None,
            Box::new(DownlinkCallback {
                sender: downlink_callback_sender,
            }),
        )
        .await
    {
        error!("Failed to add callback to mqtt runtime: {e}");
        return;
    }

    //TODO remove
    #[cfg(debug_assertions)]
    {
        trace!("Spawning send bundle after delay task");
        let state_clone = state.clone();
        tokio::spawn(async move {
            send_bundle_after_delay(state_clone).await;
        });
    }
    //end remove

    let mut api = OpenApi {
        info: Info {
            description: Some("The Spatz REST API".to_string()),
            ..Info::default()
        },
        ..OpenApi::default()
    };

    trace!("Creating Axum application");
    let app = ApiRouter::new()
        .route("/api.json", axum::routing::get(api::serve_api))
        .api_route(
            "/api/end_devices",
            aide::axum::routing::delete(api::rest_end_devices::delete_end_devices),
        )
        .api_route(
            "/api/end_devices",
            aide::axum::routing::get(api::rest_end_devices::list_end_devices),
        )
        .api_route(
            "/api/end_devices",
            aide::axum::routing::post(api::rest_end_devices::add_end_devices),
        )
        .route("/ws", axum::routing::get(api::websockets::ws_handler))
        .with_state(state)
        // Redoc route needs to be added after state as work around: https://github.com/tamasfe/aide/issues/26
        .route("/redoc", Redoc::new("/api.json").axum_route())
        .finish_api(&mut api)
        .layer(CorsLayer::permissive())
        .layer(Extension(api))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(true)),
        );

    let addr = SocketAddr::from((
        configuration.daemon.bind_addr,
        configuration.daemon.bind_port,
    ));

    trace!("Spawning Axum server on {}", addr);
    trace!("OpenAPI spec at /api.json");
    if let Err(e) = axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
    {
        error!("Failed to start axum: {e}");
    }
}

// TODO remove, only for debugging
#[allow(clippy::unwrap_used)]
#[cfg(debug_assertions)]
async fn send_bundle_after_delay(state: Arc<AppState>) {
    use rand::Rng;
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    let destination = EndDeviceId(rand::thread_rng().gen());
    let source = EndDeviceId(rand::thread_rng().gen());
    let timestamp = chrono::Utc::now();
    let initial_payload = vec![0xFF; 100];

    let primary = bp7::primary::PrimaryBlockBuilder::new()
        .source(source.try_into().unwrap())
        .destination(destination.try_into().unwrap())
        .creation_timestamp(bp7::CreationTimestamp::with_time_and_seq(
            receive_buffers::unix_ts_to_dtn_time(timestamp.timestamp() as u64),
            0,
        ))
        .lifetime(std::time::Duration::from_secs(2 * 24 * 60 * 60))
        .build()
        .expect("At time of writing, build only checks whether a destination is set");

    let canonical = bp7::canonical::new_payload_block(
        bp7::flags::BlockControlFlags::empty(),
        initial_payload.clone(),
    );
    let bp7_bundle = bp7::Bundle::new(primary, vec![canonical]);
    state.bundles_from_ws.send(bp7_bundle).await.unwrap()
}
