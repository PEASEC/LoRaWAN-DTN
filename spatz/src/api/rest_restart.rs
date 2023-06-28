//! REST API endpoint indicating whether a restart is needed to apply the configuration.

use crate::graceful_shutdown::ShutdownConditions;
use crate::AppState;
use aide::axum::IntoApiResponse;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use std::sync::Arc;
use tracing::trace;

/// Returns whether configurations have been changed which require a restart.
pub async fn get_restart_pending(State(state): State<Arc<AppState>>) -> impl IntoApiResponse {
    trace!("Restart pending request");

    Json(state.configuration.lock().await.restart_pending())
}

/// Initiates a restart.
///
/// Allways returns status code 200.
#[allow(clippy::unused_async)]
pub async fn restart(State(state): State<Arc<AppState>>) -> impl IntoApiResponse {
    trace!("Restart request");
    state
        .restart_initiator
        .initiate_shutdown(ShutdownConditions::Restart);

    StatusCode::OK
}
