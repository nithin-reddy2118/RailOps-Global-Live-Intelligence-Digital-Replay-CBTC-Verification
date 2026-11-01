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
        models::NetworkOverview,
        models::TrainOverview,
        models::BlockOverview,
        models::SignalOverview,
        models::SwitchOverview,
        models::AuthorityOverview,
        models::UnacknowledgedAlertOverview,
        models::EmergencyStopResult,
    )),
    tags((name = "control", description = "Central control room overview and commands"))
)]
pub struct ApiDoc;

pub fn build_router(state: AppState, jwt_codec: Arc<JwtCodec>) -> Router {
    Router::new()
        .route("/control/overview", get(handlers::overview))
        .route(
            "/control/emergency-stop-all",
            post(handlers::emergency_stop_all),
        )
        .merge(SwaggerUi::new("/docs").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .layer(axum::Extension(jwt_codec))
        .layer(CorsLayer::permissive())
        .with_state(state)
}
