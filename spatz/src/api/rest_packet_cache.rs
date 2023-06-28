//! REST API endpoints for the packet cache API.

use crate::configuration::PacketCacheConfig;
use crate::database::{insert_into_db, DataKey};
use crate::AppState;
use aide::axum::IntoApiResponse;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use std::sync::Arc;
use tracing::trace;

/// Returns the currently active packet cache configuration.
pub async fn get_current_packet_cache_config(
    State(state): State<Arc<AppState>>,
) -> impl IntoApiResponse {
    trace!("Current packet cache config request");
    Json(
        state
            .configuration
            .lock()
            .await
            .currently_active_configuration
            .daemon
            .packet_cache
            .clone(),
    )
}

/// Sets the packet cache configuration for the next restart of the instance.
///
/// Returns an internal server error if the configuration could not be saved to the database.
pub async fn set_next_packet_cache_config(
    State(state): State<Arc<AppState>>,
    Json(packet_cache_config): Json<PacketCacheConfig>,
) -> impl IntoApiResponse {
    trace!("Setting next packet cache config");
    let mut config_lock = state.configuration.lock().await;

    config_lock.next_configuration.daemon.packet_cache = packet_cache_config;
    match insert_into_db(
        DataKey::Configuration,
        &config_lock.next_configuration,
        state.db_pool.clone(),
    )
    .await
    {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// Returns the packet cache configuration used after the next restart of the instance.
pub async fn get_next_packet_cache_config(
    State(state): State<Arc<AppState>>,
) -> impl IntoApiResponse {
    trace!("Next packet cache config request");
    Json(
        state
            .configuration
            .lock()
            .await
            .next_configuration
            .daemon
            .packet_cache
            .clone(),
    )
}

/// Returns the packet hashes currently held in the packet cache.
pub async fn get_packet_cache_contents(State(state): State<Arc<AppState>>) -> impl IntoApiResponse {
    trace!("Packet cache content request");
    Json(state.packet_cache.contents().await)
}
