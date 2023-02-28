use crate::end_device_id::ManagedEndDeviceId;
use crate::AppState;
use aide::axum::IntoApiResponse;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::trace;

/// JSON parameter and response for end device numbers.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EndDeviceNumbersJsonParameter {
    pub end_devices: Vec<String>,
}

/// Handler to delete end device numbers. Will always return HTTP 200.
pub async fn delete_end_devices(
    State(state): State<Arc<AppState>>,
    Json(end_device_ids): Json<EndDeviceNumbersJsonParameter>,
) -> impl IntoApiResponse {
    trace!("Deleting end devices: {:?}", end_device_ids.end_devices);
    let mut end_device_id_lock = state.end_device_ids.lock().expect("Lock poisoned");
    for number in end_device_ids.end_devices {
        end_device_id_lock.remove(&number.into());
    }
    StatusCode::OK
}

/// Returns all currently registered end device numbers. An empty list if no end device IDs are registered.
pub async fn list_end_devices(State(state): State<Arc<AppState>>) -> impl IntoApiResponse {
    trace!("Listing end devices");
    let end_device_id_lock = state.end_device_ids.lock().expect("Lock poisoned");
    let end_device_number: Vec<String> = end_device_id_lock
        .iter()
        .map(|end_device_id| end_device_id.number())
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
    let mut end_device_id_lock = state.end_device_ids.lock().expect("Lock poisoned");
    end_device_number
        .end_devices
        .iter()
        .map(ManagedEndDeviceId::from)
        .for_each(|end_device_id| {
            end_device_id_lock.insert(end_device_id);
        });
    StatusCode::OK
}
