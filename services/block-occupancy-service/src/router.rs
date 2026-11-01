use crate::{handlers, models, state::AppState};
use axum::{
    routing::{get, patch},
    Router,
};
use cbtc_auth::JwtCodec;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[derive(OpenApi)]
#[openapi(
    paths(),
    components(schemas(models::BlockResponse, models::ManualOccupancyRequest)),
    tags((name = "blocks", description = "Track block occupancy state"))
)]
pub struct ApiDoc;

pub fn build_router(state: AppState, jwt_codec: Arc<JwtCodec>) -> Router {
    Router::new()
        .route("/blocks", get(handlers::list_blocks))
        .route("/blocks/:id", get(handlers::get_block))
        .route(
            "/blocks/:id/occupancy",
            patch(handlers::set_manual_occupancy),
        )
        .merge(SwaggerUi::new("/docs").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .layer(axum::Extension(jwt_codec))
        .layer(CorsLayer::permissive())
        .with_state(state)
}
