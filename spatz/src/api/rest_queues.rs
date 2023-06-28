//! REST API endpoints for the message/packet queues API.

use crate::configuration::QueueConfig;
use crate::database::{insert_into_db, DataKey};
use crate::AppState;
use aide::axum::IntoApiResponse;
use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use std::sync::Arc;
use tracing::trace;

/// Returns the message buffer queue.
pub async fn get_message_buffer_queue(State(state): State<Arc<AppState>>) -> impl IntoApiResponse {
    trace!("Message buffer queue request");
    match serde_json::to_string(&(*state.queue_manager.bundle_send_buffer_queue.lock().await)) {
        Ok(message_queue) => {
            let mut headers = HeaderMap::new();
            headers.insert(
                header::CONTENT_TYPE,
                "application/json"
                    .parse()
                    .expect("Failed to build json header"),
            );
            (headers, message_queue).into_response()
        }
        Err(err) => {
            trace!(%err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Returns the relay packet queue.
pub async fn get_relay_packet_queue(State(state): State<Arc<AppState>>) -> impl IntoApiResponse {
    trace!("Relay packet queue request");
    match serde_json::to_string(&(*state.queue_manager.relay_packet_queue.lock().await)) {
        Ok(packet_queue) => {
            let mut headers = HeaderMap::new();
            headers.insert(
                header::CONTENT_TYPE,
                "application/json"
                    .parse()
                    .expect("Failed to build json header"),
            );
            (headers, packet_queue).into_response()
        }
        Err(err) => {
            trace!(%err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Returns the currently active message/packet configuration.
pub async fn get_current_queues_config(State(state): State<Arc<AppState>>) -> impl IntoApiResponse {
    trace!("Current message/packet config request");

    Json(
        state
            .configuration
            .lock()
            .await
            .currently_active_configuration
            .daemon
            .queue_config
            .clone(),
    )
}

/// Returns the message/packet configuration used after the next restart of the instance.
pub async fn get_next_queues_config(State(state): State<Arc<AppState>>) -> impl IntoApiResponse {
    trace!("Next message/packet config request");

    Json(
        state
            .configuration
            .lock()
            .await
            .next_configuration
            .daemon
            .queue_config
            .clone(),
    )
}

/// Sets the message/packet configuration for the next restart of the instance.
///
/// Returns an internal server error if the configuration could not be saved to the database.
pub async fn set_next_queues_config(
    State(state): State<Arc<AppState>>,
    Json(queue_config): Json<QueueConfig>,
) -> impl IntoApiResponse {
    trace!("Setting next message/packet config");
    let mut config_lock = state.configuration.lock().await;

    config_lock.next_configuration.daemon.queue_config = queue_config;
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
