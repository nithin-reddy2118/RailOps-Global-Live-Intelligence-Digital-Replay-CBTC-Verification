use crate::{handlers, state::AppState};
use axum::middleware::from_fn_with_state;
use axum::{
    routing::{get, post},
    Router,
};
use cbtc_auth::JwtCodec;
use std::sync::Arc;
use tower_http::cors::CorsLayer;

pub fn build_router(state: AppState, jwt_codec: Arc<JwtCodec>) -> Router {
    Router::new()
        .route("/dashboard/ws", get(handlers::websocket_handler))
        .route("/dashboard/snapshot", get(handlers::snapshot))
        .route("/dashboard/platform/event", post(handlers::platform_event))
        .route("/dashboard/stations", get(handlers::stations))
        .route("/dashboard/trains", get(handlers::trains))
        .route(
            "/dashboard/trains/:train_number/route",
            get(handlers::train_route),
        )
        .route(
            "/dashboard/replay/runs",
            get(handlers::list_runs).post(handlers::start_run),
        )
        .route("/dashboard/replay/runs/:run_id", get(handlers::get_run))
        .route(
            "/dashboard/replay/runs/:run_id/stop",
            post(handlers::stop_run),
        )
        .route(
            "/dashboard/replay/runs/:run_id/events",
            get(handlers::events),
        )
        .route(
            "/dashboard/replay/runs/:run_id/verify",
            post(handlers::verify_run),
        )
        .layer(axum::Extension(jwt_codec))
        .layer(CorsLayer::permissive())
        .layer(from_fn_with_state(state.clone(), crate::http_recording::record_http))
        .with_state(state)
}



