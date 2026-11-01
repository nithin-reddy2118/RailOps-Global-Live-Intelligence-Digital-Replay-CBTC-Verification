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
        models::RegisterRequest,
        models::LoginRequest,
        models::TokenResponse,
        models::UserResponse,
        models::AssignRoleRequest,
    )),
    tags((name = "auth", description = "Authentication & user management"))
)]
pub struct ApiDoc;

pub fn build_router(state: AppState, jwt_codec: Arc<JwtCodec>) -> Router {
    Router::new()
        .route("/auth/register", post(handlers::register))
        .route("/auth/login", post(handlers::login))
        .route("/auth/me", get(handlers::me))
        .route("/auth/users", post(handlers::create_user_as_admin))
        .route("/auth/users/:id/roles", post(handlers::assign_role))
        .merge(SwaggerUi::new("/docs").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .layer(axum::Extension(jwt_codec))
        .layer(CorsLayer::permissive())
        .with_state(state)
}
