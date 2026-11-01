use crate::{handlers, models, state::AppState};
use axum::{routing::post, Router};
use cbtc_auth::JwtCodec;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[derive(OpenApi)]
#[openapi(
    paths(),
    components(schemas(
        models::RequestAuthorityRequest,
        models::RequestAuthorityResponse,
        models::SwitchCommandRequest,
    )),
    tags((name = "interlocking", description = "Route computation, conflict checking, and switch control"))
)]
pub struct ApiDoc;

pub fn build_router(state: AppState, jwt_codec: Arc<JwtCodec>) -> Router {
    Router::new()
        .route(
            "/interlocking/request-authority",
            post(handlers::request_authority),
        )
        .route(
            "/interlocking/switches/:id/command",
            post(handlers::command_switch),
        )
        .merge(SwaggerUi::new("/docs").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .layer(axum::Extension(jwt_codec))
        .layer(CorsLayer::permissive())
        .with_state(state)
}
