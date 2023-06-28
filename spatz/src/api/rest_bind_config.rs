//! REST API endpoints for the bind config API.

use crate::configuration::BindConfig;
use crate::database::{insert_into_db, DataKey};
use crate::AppState;
use aide::axum::IntoApiResponse;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use std::sync::Arc;
use tracing::trace;

/// Sets the address and port to bind to for the next restart of the instance.
///
/// Returns an internal server error if the configuration could not be saved to the database.
pub async fn set_next_bind_config(
    State(state): State<Arc<AppState>>,
    Json(bind_config): Json<BindConfig>,
) -> impl IntoApiResponse {
    trace!("Setting next bind address config");
    let mut config_lock = state.configuration.lock().await;

    config_lock.next_configuration.daemon.bind_config = bind_config;
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

/// Returns the currently active message/packet configuration.
pub async fn get_current_bind_config(State(state): State<Arc<AppState>>) -> impl IntoApiResponse {
    trace!("Current bind config request");

    Json(
        state
            .configuration
            .lock()
            .await
            .currently_active_configuration
            .daemon
            .bind_config
            .clone(),
    )
}

/// Returns the message/packet configuration used after the next restart of the instance.
pub async fn get_next_bind_config(State(state): State<Arc<AppState>>) -> impl IntoApiResponse {
    trace!("Next bind request");

    Json(
        state
            .configuration
            .lock()
            .await
            .next_configuration
            .daemon
            .bind_config
            .clone(),
    )
}
