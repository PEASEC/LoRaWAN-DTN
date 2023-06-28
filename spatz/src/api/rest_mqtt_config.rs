//! REST API endpoints for the MQTT config API.

use crate::configuration::MqttConfig;
use crate::database::{insert_into_db, DataKey};
use crate::AppState;
use aide::axum::IntoApiResponse;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use std::sync::Arc;
use tracing::trace;

/// Returns the currently active MQTT configuration.
pub async fn get_current_mqtt_config(State(state): State<Arc<AppState>>) -> impl IntoApiResponse {
    trace!("Current MQTT config request");

    Json(
        state
            .configuration
            .lock()
            .await
            .currently_active_configuration
            .mqtt
            .clone(),
    )
}

/// Returns the MQTT configuration used after the next restart of the instance.
pub async fn get_next_mqtt_config(State(state): State<Arc<AppState>>) -> impl IntoApiResponse {
    trace!("Next MQTT config request");

    Json(
        state
            .configuration
            .lock()
            .await
            .next_configuration
            .mqtt
            .clone(),
    )
}

/// Sets the MQTT configuration for the next restart of the instance.
///
/// Returns an internal server error if the configuration could not be saved to the database.
pub async fn set_next_mqtt_config(
    State(state): State<Arc<AppState>>,
    Json(mqtt_config): Json<MqttConfig>,
) -> impl IntoApiResponse {
    trace!("Setting next MQTT config");
    let mut config_lock = state.configuration.lock().await;

    config_lock.next_configuration.mqtt = mqtt_config;
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
