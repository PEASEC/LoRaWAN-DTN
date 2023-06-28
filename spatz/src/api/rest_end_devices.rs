//! REST API endpoints for the end device API.

use crate::database::{insert_into_db, DataKey};
use crate::end_device_id::ManagedEndDeviceId;
use crate::error::DbError;
use crate::{AppState, SpatzConfig};
use aide::axum::IntoApiResponse;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::trace;

/// JSON parameter and response for end device numbers.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EndDeviceNumbersJsonParameter {
    /// End devices.
    pub end_devices: Vec<String>,
}

/// Handler to delete end device numbers. Will always return HTTP 200.
pub async fn delete_end_devices(
    State(state): State<Arc<AppState>>,
    Json(end_device_ids): Json<EndDeviceNumbersJsonParameter>,
) -> impl IntoApiResponse {
    trace!("Deleting end devices: {:?}", end_device_ids.end_devices);
    let mut end_device_id_lock = state.end_device_ids.lock().await;
    for number in end_device_ids.end_devices {
        end_device_id_lock.remove(&number.into());
    }
    let updated_end_device_ids = end_device_id_lock.clone();
    if let Err(err) = update_config_end_device_ids(
        updated_end_device_ids,
        state.configuration.clone(),
        state.db_pool.clone(),
    )
    .await
    {
        trace!("Error writing config to database: {err}");
    }

    StatusCode::OK
}

/// Returns all currently registered end device numbers. An empty list if no end device IDs are registered.
pub async fn list_end_devices(State(state): State<Arc<AppState>>) -> impl IntoApiResponse {
    trace!("Listing end devices");
    let end_device_id_lock = state.end_device_ids.lock().await;
    let end_device_number: Vec<String> = end_device_id_lock
        .iter()
        .map(ManagedEndDeviceId::phone_number)
        .collect();
    Json(EndDeviceNumbersJsonParameter {
        end_devices: end_device_number,
    })
}

/// Adds the in the parameter specified end device numbers to the daemon. Always returns HTTP 200.
pub async fn add_end_devices(
    State(state): State<Arc<AppState>>,
    Json(end_device_number): Json<EndDeviceNumbersJsonParameter>,
) -> impl IntoApiResponse {
    trace!("Adding end devices: {:?}", end_device_number.end_devices);
    let mut end_device_id_lock = state.end_device_ids.lock().await;
    end_device_number
        .end_devices
        .iter()
        .map(ManagedEndDeviceId::from)
        .for_each(|end_device_id| {
            end_device_id_lock.insert(end_device_id);
        });
    let updated_end_device_ids = end_device_id_lock.clone();
    if let Err(err) = update_config_end_device_ids(
        updated_end_device_ids,
        state.configuration.clone(),
        state.db_pool.clone(),
    )
    .await
    {
        trace!("Error writing config to database: {err}");
    }

    StatusCode::OK
}

/// Updates end device IDs in the global config and the database.
///
/// # Error
///
/// Returns an error if the database returned an error.
async fn update_config_end_device_ids(
    end_device_ids: HashSet<ManagedEndDeviceId>,
    config: Arc<Mutex<SpatzConfig>>,
    db_pool: SqlitePool,
) -> Result<(), DbError> {
    let updated_end_device_ids = end_device_ids.into_iter().fold(Vec::new(), |mut acc, id| {
        acc.push(id.phone_number());
        acc
    });
    let mut config_lock = config.lock().await;
    config_lock
        .currently_active_configuration
        .daemon
        .end_device_ids = updated_end_device_ids.clone();
    config_lock.next_configuration.daemon.end_device_ids = updated_end_device_ids;
    insert_into_db(
        DataKey::Configuration,
        &config_lock.next_configuration,
        db_pool,
    )
    .await
}
