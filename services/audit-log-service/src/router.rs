use crate::{handlers, models, state::AppState};
use axum::{routing::get, Router};
use cbtc_auth::JwtCodec;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[derive(OpenApi)]
#[openapi(paths(), components(schemas(models::AuditEntryResponse)), tags((name = "audit", description = "Platform audit trail")))]
pub struct ApiDoc;

pub fn build_router(state: AppState, jwt_codec: Arc<JwtCodec>) -> Router {
    Router::new()
        .route("/audit/recent", get(handlers::recent))
        .route("/audit/causality/:id", get(handlers::by_causality))
        .merge(SwaggerUi::new("/docs").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .layer(axum::Extension(jwt_codec))
        .layer(CorsLayer::permissive())
        .with_state(state)
}
