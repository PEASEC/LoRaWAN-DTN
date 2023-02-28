use aide::axum::IntoApiResponse;
use aide::openapi::OpenApi;
use axum::{Extension, Json};

pub mod rest_end_devices;
pub mod websockets;

/// Serves the generated OpenAPI spec.
pub async fn serve_api(Extension(api): Extension<OpenApi>) -> impl IntoApiResponse {
    Json(api)
}
