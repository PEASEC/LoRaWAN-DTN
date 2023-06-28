//! Methods used when starting the Spatz application.

use crate::api::create_api;
use crate::bundle_processing::bundles_processor_task;
use crate::configuration::{CliParameters, Configuration, RoutingAlgorithmConfig};
use crate::database::{fetch_from_db, insert_into_db, DataKey};
use crate::duty_cycle_manager::{DownlinkCallback, DutyCycleManager};
use crate::end_device_id::{EndDeviceId, ManagedEndDeviceId};
use crate::gateway_ids_manager::GatewayIdsManager;
use crate::graceful_shutdown::{ShutdownAgent, ShutdownConditions, ShutdownInitiator};
use crate::packet_cache::PacketCache;
use crate::packet_queue_manager::QueueManager;
use crate::routing::{Flooding, RoutingAlgorithm};
use crate::uplink_processing::UplinkCallback;
use crate::{
    duty_cycle_manager, packet_cache, receive_buffers, uplink_processing, AppState, SpatzConfig,
};
use axum::Router;
use chirpstack_api_wrapper::ChirpStackApi;
use clap::Parser;
use config::Config;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::SqlitePool;
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, Mutex};
use tracing::{error, instrument, trace};

/// Creates the database connection and handles the configuration parsing.
pub async fn database_and_config(cli_parameters: &CliParameters) -> (SqlitePool, Configuration) {
    let db_pool = SqlitePool::connect_with(
        SqliteConnectOptions::from_str(&cli_parameters.db_url)
            .expect("Failed to create DB connection from URL")
            .create_if_missing(true),
    )
    .await
    .expect("Failed to open DB file");

    trace!("Running DB migrations");
    sqlx::migrate!()
        .run(&db_pool)
        .await
        .expect("Failed to run DB migrations");

    trace!("Building configuration");
    let configuration: Configuration =
        if let Ok(configuration) = fetch_from_db(DataKey::Configuration, db_pool.clone()).await {
            trace!("Using database configuration");
            configuration
        } else {
            trace!("Using configuration file");
            let configuration = Config::builder()
                .add_source(config::File::with_name(&cli_parameters.config_file_path))
                .build()
                .expect("Failed to build config");

            trace!("Deserializing configuration from file");
            let configuration = configuration
                .try_deserialize::<Configuration>()
                .expect("Failed to deserialize configuration");
            insert_into_db(DataKey::Configuration, &configuration, db_pool.clone())
                .await
                .expect("Failed to insert configuration into database");
            configuration
        };
    (db_pool, configuration)
}

/// Starts all parts of the application.
#[allow(clippy::too_many_lines)]
pub async fn start_app(
    shutdown_agent: ShutdownAgent,
    shutdown_initiator: ShutdownInitiator,
) -> Result<Arc<AppState>, ()> {
    trace!("Parsing cli parameters");
    let cli_parameters = CliParameters::parse();

    let (db_pool, configuration) = database_and_config(&cli_parameters).await;

    trace!("Creating channels");
    let (bundles_from_ws_tx, bundles_from_ws_rx) = mpsc::channel(10);
    let (bundles_to_ws_tx, _) = broadcast::channel(10);
    let (uplink_callback_tx, uplink_callback_rx) = mpsc::channel(10);
    let (relay_tx, relay_rx) = mpsc::channel(10);
    let (bundle_send_buffer_tx, bundle_send_buffer_rx) = mpsc::channel(10);
    let (downlink_callback_tx, downlink_callback_rx) = mpsc::channel(10);
    let (mqtt_connection_error_tx, mqtt_connection_error_rx) = broadcast::channel(10);

    trace!("Creating runtime");
    let mut runtime = match chirpstack_gwb_integration::runtime::Runtime::new(
        &configuration.mqtt.client_id,
        &configuration.mqtt.url,
        configuration.mqtt.port,
        Some(mqtt_connection_error_tx),
    )
    .await
    {
        Ok(runtime) => runtime,
        Err(e) => {
            error!("Failed to create runtime: {e}");
            return Err(());
        }
    };

    trace!("Adding universal uplink callback to runtime");
    if let Err(e) = runtime
        .add_event_up_callback(None, Box::new(UplinkCallback { uplink_callback_tx }))
        .await
    {
        error!("Failed to add callback to mqtt runtime: {e}");
        return Err(());
    }

    trace!("Adding universal downlink callback to runtime");
    if let Err(e) = runtime
        .add_command_down_callback(
            None,
            Box::new(DownlinkCallback {
                downlink_callback_tx,
            }),
        )
        .await
    {
        error!("Failed to add callback to mqtt runtime: {e}");
        return Err(());
    }

    trace!("Creating ChirpStack API info");
    let chirpstack_api = ChirpStackApi {
        url: configuration.chirpstack_api.url.clone(),
        port: configuration.chirpstack_api.port,
        api_token: configuration.chirpstack_api.api_token.clone(),
        tenant_id: configuration.chirpstack_api.tenant_id.clone(),
    };

    trace!("Fetching packet cache data from database");
    let packet_cache_data = if let Ok(packet_cache_data) =
        fetch_from_db(DataKey::PacketCacheData, db_pool.clone()).await
    {
        trace!("Fetch packet cache data from database");
        packet_cache_data
    } else {
        HashMap::new()
    };

    trace!("Creating packet cache");
    let packet_cache = PacketCache::new(
        packet_cache_data,
        configuration.daemon.packet_cache.timeout_minutes,
        configuration.daemon.packet_cache.cleanup_interval_seconds,
        configuration.daemon.packet_cache.reset_timeout,
    );

    trace!("Calculating end device IDs");
    let end_device_ids: HashSet<ManagedEndDeviceId> = configuration
        .daemon
        .end_device_ids
        .iter()
        .map(ManagedEndDeviceId::from)
        .collect();

    trace!("Fetching duty cycle data from database");
    let duty_cycle_data =
        if let Ok(duty_cycle_data) = fetch_from_db(DataKey::DutyCycleData, db_pool.clone()).await {
            trace!("Fetch duty cycle data from database");
            duty_cycle_data
        } else {
            HashMap::new()
        };

    trace!("Creating duty cycle manager");
    let duty_cycle_manager = Arc::new(Mutex::new(DutyCycleManager::new(duty_cycle_data)));

    trace!("Fetching message buffers and relay messages from database");
    let relay_packet_queue = if let Ok(relay_packet_queue) =
        fetch_from_db(DataKey::RelayMessages, db_pool.clone()).await
    {
        trace!("Fetch message buffers and relay messages from database");
        Arc::new(Mutex::new(relay_packet_queue))
    } else {
        Arc::new(Mutex::new(Vec::new()))
    };
    let bundle_send_buffer_queue = if let Ok(bundle_send_buffer_queue) =
        fetch_from_db(DataKey::MessageBuffers, db_pool.clone()).await
    {
        Arc::new(Mutex::new(bundle_send_buffer_queue))
    } else {
        Arc::new(Mutex::new(Vec::new()))
    };

    trace!("Creating queue manager");
    let queue_manager = Arc::new(QueueManager::new(
        relay_packet_queue,
        configuration.daemon.queue_config.relay_queue_size,
        bundle_send_buffer_queue,
        configuration.daemon.queue_config.bundle_queue_size,
    ));

    trace!("Creating gateway IDs manager");
    let gateway_ids_manager = GatewayIdsManager::new(std::time::Duration::from_secs(60));

    trace!("Creating routing algorithm");
    let mut routing_algo = Box::new(match &configuration.daemon.routing_algorithm_config {
        RoutingAlgorithmConfig::Flooding(config) => {
            Flooding::new(std::time::Duration::from_secs(config.periodic_send_delay))
        }
    });
    // Provides a shutdown agent to the routing algorithm.
    routing_algo.provide_shutdown_agent(shutdown_agent.clone());

    let spatz_config = SpatzConfig {
        next_configuration: configuration.clone(),
        currently_active_configuration: configuration.clone(),
    };

    trace!("Creating state");
    let state = Arc::new(AppState {
        bundles_to_ws: bundles_to_ws_tx,
        bundles_from_ws: bundles_from_ws_tx,
        runtime: runtime.clone(),
        end_device_ids: Arc::new(Mutex::new(end_device_ids)),
        chirpstack_api,
        packet_cache,
        duty_cycle_manager,
        queue_manager,
        gateway_ids_manager,
        routing_algo,
        db_pool: db_pool.clone(),
        restart_initiator: shutdown_initiator,
        configuration: Arc::new(Mutex::new(spatz_config)),
    });

    let addr = SocketAddr::from((
        configuration.daemon.bind_config.bind_addr,
        configuration.daemon.bind_config.bind_port,
    ));

    trace!("Spawn flooding task");
    let flooding_shutdown_agent = shutdown_agent.clone();
    let state_clone = state.clone();
    tokio::spawn(async move {
        let state_clone1 = state_clone.clone();
        state_clone
            .routing_algo
            .routing_task(state_clone1, flooding_shutdown_agent)
            .await;
    });

    trace!("Spawn MQTT connection error listener");
    let mqtt_shutdown_agent = shutdown_agent.clone();
    tokio::spawn(async move {
        mqtt_connection_error_task(mqtt_connection_error_rx, mqtt_shutdown_agent).await;
    });

    trace!("Spawn runtime shutdown task");
    let runtime_shutdown_agent = shutdown_agent.clone();
    let runtime_clone = runtime.clone();
    tokio::spawn(async move { runtime_shutdown_task(runtime_clone, runtime_shutdown_agent).await });

    trace!("Spawning QueueManager::collect_send_items task");
    let consolidate_send_items_shutdown_agent = shutdown_agent.clone();
    let queue_manager_clone = state.queue_manager.clone();
    tokio::spawn(async move {
        queue_manager_clone
            .collect_send_items_task(
                relay_rx,
                bundle_send_buffer_rx,
                consolidate_send_items_shutdown_agent,
            )
            .await;
    });

    trace!("Spawning packet cache clean task");
    let state_clone = state.clone();
    let cache_clean_task_shutdown_agent = shutdown_agent.clone();
    tokio::spawn(async move {
        packet_cache::cache_clean_task(state_clone, cache_clean_task_shutdown_agent).await;
    });

    trace!("Spawning duty cycle manager callback task");
    let state_clone = state.clone();
    let downlink_duty_cycle_collector_shutdown_agent = shutdown_agent.clone();
    tokio::spawn(async move {
        duty_cycle_manager::downlink_duty_cycle_collector_task(
            downlink_callback_rx,
            state_clone,
            downlink_duty_cycle_collector_shutdown_agent,
        )
        .await;
    });

    trace!("Spawning uplink processor task");
    let state_clone = state.clone();
    let uplink_processor_shutdown_agent = shutdown_agent.clone();
    tokio::spawn(async move {
        uplink_processing::uplink_processor_task(
            uplink_callback_rx,
            relay_tx,
            state_clone,
            uplink_processor_shutdown_agent,
        )
        .await;
    });

    trace!("Spawning bundles processor task");
    let bundles_processor_shutdown_agent = shutdown_agent.clone();
    tokio::spawn(async move {
        bundles_processor_task(
            bundles_from_ws_rx,
            bundle_send_buffer_tx,
            bundles_processor_shutdown_agent,
        )
        .await;
    });

    trace!("Spawning gateway manager update task");
    let gateway_manager_shutdown_agent = shutdown_agent.clone();
    let state_clone = state.clone();
    tokio::spawn(async move {
        let state_clone2 = state_clone.clone();
        state_clone
            .gateway_ids_manager
            .update_gateways(state_clone2, gateway_manager_shutdown_agent)
            .await;
    });

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

    trace!("Spawning Axum server on {}", addr);
    trace!("OpenAPI spec at /api.json");
    let axum_server_shutdown_agent = shutdown_agent.clone();
    tokio::spawn({
        let state = state.clone();
        async move {
            axum_task(create_api(state), addr, axum_server_shutdown_agent).await;
        }
    });
    Ok(state)
}

/// Async task to receive MQTT connection errors.
#[instrument(skip_all)]
async fn mqtt_connection_error_task(
    mut mqtt_connection_error_rx: broadcast::Receiver<String>,
    mut shutdown_agent: ShutdownAgent,
) {
    trace!("Starting up");
    let mut mqtt_shutdown_agent_clone = shutdown_agent.clone();
    loop {
        tokio::select! {
            mqtt_connection_error = mqtt_connection_error_rx.recv() => {
                if let Ok(err_msg) = mqtt_connection_error {
                    error!(
                        "More than 3 MQTT connection errors within 30 seconds. Last error was: {err_msg}"
                    );
                    mqtt_shutdown_agent_clone.initiate_shutdown(ShutdownConditions::MqttError);
                }
            },
            _ = shutdown_agent.await_shutdown() => {
                trace!("Shutting down");
                return
            }
        }
    }
}

/// Task to shut down the ChirpStack GWB integration runtime.
#[instrument(skip_all)]
async fn runtime_shutdown_task(
    mut runtime: chirpstack_gwb_integration::runtime::Runtime,
    mut shutdown_agent: ShutdownAgent,
) {
    trace!("Starting up");
    shutdown_agent.await_shutdown().await;
    runtime.stop_event_loop();
    trace!("Shutting down");
}

/// Async task to run axum server.
#[instrument(skip_all)]
async fn axum_task(axum_router: Router, addr: SocketAddr, mut shutdown_agent: ShutdownAgent) {
    trace!("Starting up");
    if let Err(e) = axum::Server::bind(&addr)
        .serve(axum_router.into_make_service())
        .with_graceful_shutdown(async {
            shutdown_agent.await_shutdown().await;
            trace!("Shutting down");
        })
        .await
    {
        error!("Failed to start axum: {e}");
        shutdown_agent.initiate_shutdown(ShutdownConditions::AxumStartFailed);
    };
}

// TODO remove, only for debugging
#[cfg(debug_assertions)]
#[allow(clippy::unwrap_used)]
#[instrument(skip_all)]
async fn send_bundle_after_delay(state: Arc<AppState>) {
    use rand::Rng;
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    let destination = EndDeviceId(rand::thread_rng().gen());

    let _rak2_dst = EndDeviceId::from(ManagedEndDeviceId::from("3234567890".to_owned()));
    //let destination = _rak2_dst;

    let source = EndDeviceId(rand::thread_rng().gen());
    let timestamp = chrono::Utc::now();
    let initial_payload = vec![0xFF; 100];

    let primary = bp7::primary::PrimaryBlockBuilder::new()
        .source(source.try_into().unwrap())
        .destination(destination.try_into().unwrap())
        .creation_timestamp(bp7::CreationTimestamp::with_time_and_seq(
            receive_buffers::unix_ts_to_dtn_time(u64::try_from(timestamp.timestamp()).unwrap()),
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
    state.bundles_from_ws.send(bp7_bundle).await.unwrap();
    trace!("send_bundle_after_delay: exit");
}
