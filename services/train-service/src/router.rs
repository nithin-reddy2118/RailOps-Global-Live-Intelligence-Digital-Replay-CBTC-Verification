use crate::{handlers, models, state::AppState};
use axum::{
    routing::{get, patch, post},
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
        models::CreateTrainRequest,
        models::TrainResponse,
        models::PositionReportRequest,
        models::StateChangeRequest,
    )),
    tags((name = "trains", description = "Train fleet management and telemetry ingestion"))
)]
pub struct ApiDoc;

pub fn build_router(state: AppState, jwt_codec: Arc<JwtCodec>) -> Router {
    Router::new()
        .route(
            "/trains",
            post(handlers::create_train).get(handlers::list_trains),
        )
        .route("/trains/:id", get(handlers::get_train))
        .route(
            "/trains/:id/position-reports",
            post(handlers::record_position_report),
        )
        .route("/trains/:id/heartbeat", post(handlers::heartbeat))
        .route("/trains/:id/state", patch(handlers::change_state))
        .merge(SwaggerUi::new("/docs").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .layer(axum::Extension(jwt_codec))
        .layer(CorsLayer::permissive())
        .with_state(state)
}
