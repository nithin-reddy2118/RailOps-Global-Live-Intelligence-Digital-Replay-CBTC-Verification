use crate::proxy::{self, ProxyState};
use axum::{routing::any, Router};
use tower_http::cors::CorsLayer;

/// Builds the API gateway router.
///
/// The gateway provides a single entry point for the platform services.
/// Request-rate limiting can be added later with an Axum-compatible
/// middleware layer once a client-identity strategy is defined.
pub fn build_router(http: reqwest::Client, _requests_per_second: u64) -> Router {
    let state = ProxyState { http };

    Router::new()
        .route("/*path", any(proxy::proxy))
        .route("/", any(proxy::proxy))
        .layer(CorsLayer::permissive())
        .with_state(state)
}
