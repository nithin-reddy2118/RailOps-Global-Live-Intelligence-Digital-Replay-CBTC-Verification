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
    components(schemas(models::StationResponse, models::PlatformSummary, models::PlatformStatus)),
    tags((name = "passenger-info", description = "Station, platform, and arrival information"))
)]
pub struct ApiDoc;

pub fn build_router(state: AppState, jwt_codec: Arc<JwtCodec>) -> Router {
    Router::new()
        .route("/stations", get(handlers::list_stations))
        .route("/platforms/:id/status", get(handlers::platform_status))
        .merge(SwaggerUi::new("/docs").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .layer(axum::Extension(jwt_codec))
        .layer(CorsLayer::permissive())
        .with_state(state)
}
