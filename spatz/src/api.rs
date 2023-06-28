//! The REST/WS API of Spatz.

use crate::AppState;
use aide::axum::{ApiRouter, IntoApiResponse};
use aide::openapi::{Info, OpenApi};
use aide::redoc::Redoc;
use axum::{Extension, Json, Router};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::trace::{DefaultMakeSpan, TraceLayer};
use tracing::trace;

pub mod rest_bind_config;
pub mod rest_chirpstack_config;
pub mod rest_duty_cycle;
pub mod rest_end_devices;
pub mod rest_mqtt_config;
pub mod rest_packet_cache;
pub mod rest_queues;
pub mod rest_restart;
pub mod websockets;

/// Serves the generated OpenAPI spec.
#[allow(clippy::unused_async)]
pub async fn serve_api(Extension(api): Extension<OpenApi>) -> impl IntoApiResponse {
    Json(api)
}

/// Returns the Spatz API.
#[allow(clippy::too_many_lines)]
pub fn create_api(state: Arc<AppState>) -> Router {
    let mut api = OpenApi {
        info: Info {
            description: Some("The Spatz REST API".to_string()),
            ..Info::default()
        },
        ..OpenApi::default()
    };

    trace!("Creating Axum application");
    ApiRouter::new()
        .route("/api.json", axum::routing::get(serve_api))
        // Config
        // Bind
        .api_route(
            "/api/config/current/bind",
            aide::axum::routing::get(rest_bind_config::get_current_bind_config),
        )
        .api_route(
            "/api/config/next/bind",
            aide::axum::routing::get(rest_bind_config::get_next_bind_config),
        )
        .api_route(
            "/api/config/next/bind",
            aide::axum::routing::post(rest_bind_config::set_next_bind_config),
        )
        // ChirpStack
        .api_route(
            "/api/config/current/chirpstack",
            aide::axum::routing::get(rest_chirpstack_config::get_current_chirpstack_config),
        )
        .api_route(
            "/api/config/next/chirpstack",
            aide::axum::routing::get(rest_chirpstack_config::get_next_chirpstack_config),
        )
        .api_route(
            "/api/config/next/chirpstack",
            aide::axum::routing::post(rest_chirpstack_config::set_next_chirpstack_config),
        )
        // MQTT
        .api_route(
            "/api/config/current/mqtt",
            aide::axum::routing::get(rest_mqtt_config::get_current_mqtt_config),
        )
        .api_route(
            "/api/config/next/mqtt",
            aide::axum::routing::get(rest_mqtt_config::get_next_mqtt_config),
        )
        .api_route(
            "/api/config/next/mqtt",
            aide::axum::routing::post(rest_mqtt_config::set_next_mqtt_config),
        )
        // Packet cache
        .api_route(
            "/api/config/current/packet_cache",
            aide::axum::routing::get(rest_packet_cache::get_current_packet_cache_config),
        )
        .api_route(
            "/api/config/next/packet_cache",
            aide::axum::routing::get(rest_packet_cache::get_next_packet_cache_config),
        )
        .api_route(
            "/api/config/next/packet_cache",
            aide::axum::routing::post(rest_packet_cache::set_next_packet_cache_config),
        )
        // Message/packet queue
        .api_route(
            "/api/config/current/queues",
            aide::axum::routing::get(rest_queues::get_current_queues_config),
        )
        .api_route(
            "/api/config/next/queues",
            aide::axum::routing::get(rest_queues::get_next_queues_config),
        )
        .api_route(
            "/api/config/next/queues",
            aide::axum::routing::post(rest_queues::set_next_queues_config),
        )
        // Stats
        .api_route(
            "/api/stats/packet_cache",
            aide::axum::routing::get(rest_packet_cache::get_packet_cache_contents),
        )
        .api_route(
            "/api/stats/message_queue",
            aide::axum::routing::get(rest_queues::get_message_buffer_queue),
        )
        .api_route(
            "/api/stats/relay_packet_queue",
            aide::axum::routing::get(rest_queues::get_relay_packet_queue),
        )
        .api_route(
            "/api/stats/duty_cycle",
            aide::axum::routing::get(rest_duty_cycle::get_duty_cycle_stats),
        )
        // End devices
        .api_route(
            "/api/end_devices",
            aide::axum::routing::delete(rest_end_devices::delete_end_devices),
        )
        .api_route(
            "/api/end_devices",
            aide::axum::routing::get(rest_end_devices::list_end_devices),
        )
        .api_route(
            "/api/end_devices",
            aide::axum::routing::post(rest_end_devices::add_end_devices),
        )
        // Restart
        .api_route(
            "/api/restart_pending",
            aide::axum::routing::get(rest_restart::get_restart_pending),
        )
        .api_route(
            "/api/restart",
            aide::axum::routing::post(rest_restart::restart),
        )
        .route("/ws", axum::routing::get(websockets::ws_handler))
        .with_state(state)
        // Redoc route needs to be added after state as work around: https://github.com/tamasfe/aide/issues/26
        .route("/redoc", Redoc::new("/api.json").axum_route())
        .finish_api(&mut api)
        .layer(CorsLayer::permissive())
        .layer(Extension(api))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(true)),
        )
}
