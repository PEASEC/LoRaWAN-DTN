//! REST API endpoints for the duty cycle API.

use crate::AppState;
use aide::axum::IntoApiResponse;
use axum::extract::State;
use axum::Json;
use std::sync::Arc;
use tracing::trace;

/// Returns the currently active packet cache configuration.
pub async fn get_duty_cycle_stats(State(state): State<Arc<AppState>>) -> impl IntoApiResponse {
    trace!("Duty cycle stats request");

    Json(state.duty_cycle_manager.lock().await.stats())
}
