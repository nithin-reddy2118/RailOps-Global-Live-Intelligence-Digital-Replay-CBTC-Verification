use crate::{handlers, state::AppState};
use axum::{routing::get, Router};
use cbtc_auth::JwtCodec;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[derive(OpenApi)]
#[openapi(
    paths(),
    components(schemas(handlers::SignalStateResponse)),
    tags((name = "wayside", description = "Signal aspect state and fault simulation"))
)]
pub struct ApiDoc;

pub fn build_router(state: AppState, jwt_codec: Arc<JwtCodec>) -> Router {
    Router::new()
        .route("/wayside/signals/:id", get(handlers::get_signal))
        .route(
            "/wayside/signals/:id/inject-fault",
            axum::routing::post(handlers::inject_fault),
        )
        .route(
            "/wayside/signals/:id/clear-fault",
            axum::routing::post(handlers::clear_fault),
        )
        .merge(SwaggerUi::new("/docs").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .layer(axum::Extension(jwt_codec))
        .layer(CorsLayer::permissive())
        .with_state(state)
}
