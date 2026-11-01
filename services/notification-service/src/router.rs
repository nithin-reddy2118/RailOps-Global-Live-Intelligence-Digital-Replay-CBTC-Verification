use crate::{handlers, models, state::AppState};
use axum::{
    routing::{get, post},
    Router,
};
use cbtc_auth::JwtCodec;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[derive(OpenApi)]
#[openapi(paths(), components(schemas(models::AlertResponse, models::AlertSeverity)), tags((name = "alerts", description = "Operational alerts")))]
pub struct ApiDoc;

pub fn build_router(state: AppState, jwt_codec: Arc<JwtCodec>) -> Router {
    Router::new()
        .route("/alerts", get(handlers::list_recent))
        .route("/alerts/unacknowledged", get(handlers::list_unacknowledged))
        .route("/alerts/:id/acknowledge", post(handlers::acknowledge))
        .merge(SwaggerUi::new("/docs").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .layer(axum::Extension(jwt_codec))
        .layer(CorsLayer::permissive())
        .with_state(state)
}
