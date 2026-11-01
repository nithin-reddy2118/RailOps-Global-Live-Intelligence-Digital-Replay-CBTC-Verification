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
#[openapi(
    paths(),
    components(schemas(
        models::GrantAuthorityRequest,
        models::AuthorityResponse,
        models::RevokeAuthorityRequest,
    )),
    tags((name = "authorities", description = "Movement authority lifecycle"))
)]
pub struct ApiDoc;

pub fn build_router(state: AppState, jwt_codec: Arc<JwtCodec>) -> Router {
    Router::new()
        .route(
            "/authorities",
            post(handlers::grant_authority).get(handlers::list_authorities),
        )
        .route("/authorities/:id", get(handlers::get_authority))
        .route("/authorities/:id/revoke", post(handlers::revoke_authority))
        .route(
            "/authorities/train/:train_id/active",
            get(handlers::get_active_for_train),
        )
        .merge(SwaggerUi::new("/docs").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .layer(axum::Extension(jwt_codec))
        .layer(CorsLayer::permissive())
        .with_state(state)
}
