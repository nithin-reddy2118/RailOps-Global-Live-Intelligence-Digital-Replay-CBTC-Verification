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
    components(schemas(models::OpenTicketRequest, models::UpdateTicketRequest, models::TicketResponse, models::TicketStatus)),
    tags((name = "maintenance", description = "Maintenance ticket lifecycle"))
)]
pub struct ApiDoc;

pub fn build_router(state: AppState, jwt_codec: Arc<JwtCodec>) -> Router {
    Router::new()
        .route(
            "/maintenance/tickets",
            post(handlers::open_ticket).get(handlers::list_all),
        )
        .route("/maintenance/tickets/open", get(handlers::list_open))
        .route(
            "/maintenance/tickets/:id",
            get(handlers::get_ticket).patch(handlers::update_ticket),
        )
        .merge(SwaggerUi::new("/docs").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .layer(axum::Extension(jwt_codec))
        .layer(CorsLayer::permissive())
        .with_state(state)
}
