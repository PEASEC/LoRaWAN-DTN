//! REST API endpoints for the ChirpStack config API.

use crate::configuration::ChirpStackApiConfig;
use crate::database::{insert_into_db, DataKey};
use crate::AppState;
use aide::axum::IntoApiResponse;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use std::sync::Arc;
use tracing::trace;

/// Returns the currently active ChirpStack configuration.
pub async fn get_current_chirpstack_config(
    State(state): State<Arc<AppState>>,
) -> impl IntoApiResponse {
    trace!("Current ChirpStack config request");

    Json(
        state
            .configuration
            .lock()
            .await
            .currently_active_configuration
            .chirpstack_api
            .clone(),
    )
}

/// Returns the ChirpStack configuration used after the next restart of the instance.
pub async fn get_next_chirpstack_config(
    State(state): State<Arc<AppState>>,
) -> impl IntoApiResponse {
    trace!("Next ChirpStack config request");

    Json(
        state
            .configuration
            .lock()
            .await
            .next_configuration
            .chirpstack_api
            .clone(),
    )
}

/// Sets the ChirpStack configuration for the next restart of the instance.
///
/// Returns an internal server error if the configuration could not be saved to the database.
pub async fn set_next_chirpstack_config(
    State(state): State<Arc<AppState>>,
    Json(chirpstack_config): Json<ChirpStackApiConfig>,
) -> impl IntoApiResponse {
    trace!("Setting next ChirpStack config");
    let mut config_lock = state.configuration.lock().await;

    config_lock.next_configuration.chirpstack_api = chirpstack_config;
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
