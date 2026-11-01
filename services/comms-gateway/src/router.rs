use crate::{handlers, models, state::AppState};
use axum::{routing::post, Router};
use cbtc_auth::JwtCodec;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[derive(OpenApi)]
#[openapi(paths(), components(schemas(models::PositionReportRequest)), tags((name = "comms", description = "Simulated train-to-wayside communication link")))]
pub struct ApiDoc;

pub fn build_router(state: AppState, jwt_codec: Arc<JwtCodec>) -> Router {
    Router::new()
        .route(
            "/comms/trains/:id/position-reports",
            post(handlers::relay_position_report),
        )
        .route(
            "/comms/trains/:id/heartbeat",
            post(handlers::relay_heartbeat),
        )
        .route(
            "/comms/trains/:id/blackout/start",
            post(handlers::start_blackout),
        )
        .route(
            "/comms/trains/:id/blackout/stop",
            post(handlers::stop_blackout),
        )
        .merge(SwaggerUi::new("/docs").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .layer(axum::Extension(jwt_codec))
        .layer(CorsLayer::permissive())
        .with_state(state)
}
