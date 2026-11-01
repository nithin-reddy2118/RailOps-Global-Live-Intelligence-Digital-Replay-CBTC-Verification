//! Tracing (structured logs) and Prometheus metrics setup, identical
//! across all fifteen services so log shape and metric naming stay
//! consistent for Grafana dashboards.

use axum::{routing::get, Router};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Initialize JSON-structured tracing to stdout, honoring `RUST_LOG` if
/// set and defaulting to `info` otherwise. Call once at the very start
/// of each service's `main`.
pub fn init_tracing(service_name: &str) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(filter)
        .with(
            fmt::layer()
                .json()
                .with_current_span(true)
                .with_target(true),
        )
        .init();

    tracing::info!(service = service_name, "tracing initialized");
}

/// Build the Prometheus recorder/exporter and return a handle whose
/// `render()` produces the text-format metrics body for the `/metrics`
/// endpoint every service exposes.
pub fn init_metrics() -> PrometheusHandle {
    PrometheusBuilder::new()
        .install_recorder()
        .expect("failed to install Prometheus recorder")
}

/// Standard `/health`, `/ready`, and `/metrics` routes merged into a
/// service's router. `/health` is a liveness check (process is up);
/// `/ready` should be composed by the caller with actual dependency
/// checks (DB, Redis, MQ) via `.route("/ready", get(custom_handler))`
/// applied *after* merging this router, since Axum lets later route
/// registrations for the same path override earlier ones is NOT
/// guaranteed — callers should instead pass their own ready handler in
/// rather than override.
pub fn observability_routes(metrics_handle: PrometheusHandle) -> Router {
    Router::new()
        .route("/health", get(|| async { "OK" }))
        .route(
            "/metrics",
            get(move || {
                let handle = metrics_handle.clone();
                async move { handle.render() }
            }),
        )
        .layer(TraceLayer::new_for_http())
}
