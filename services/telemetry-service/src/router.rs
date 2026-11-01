use crate::{handlers, models, state::AppState};
use axum::{routing::get, Router};
use cbtc_auth::JwtCodec;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[derive(OpenApi)]
#[openapi(
    paths(),
    components(schemas(models::PositionReportPoint, models::TrainTelemetrySummary)),
    tags((name = "telemetry", description = "Historical train telemetry queries"))
)]
pub struct ApiDoc;

pub fn build_router(state: AppState, jwt_codec: Arc<JwtCodec>) -> Router {
    Router::new()
        .route("/telemetry/trains/:id/history", get(handlers::history))
        .route("/telemetry/trains/:id/latest", get(handlers::latest))
        .route("/telemetry/trains/:id/summary", get(handlers::summary))
        .merge(SwaggerUi::new("/docs").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .layer(axum::Extension(jwt_codec))
        .layer(CorsLayer::permissive())
        .with_state(state)
}
