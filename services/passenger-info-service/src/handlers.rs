use crate::{
    models::{PlatformStatus, StationResponse},
    repository::PassengerInfoRepository,
    state::AppState,
};
use axum::{
    extract::{Path, State},
    Json,
};
use cbtc_errors::AppResult;
use uuid::Uuid;

pub async fn list_stations(State(state): State<AppState>) -> AppResult<Json<Vec<StationResponse>>> {
    let repo = PassengerInfoRepository::new(&state.pool);
    Ok(Json(repo.list_stations().await?))
}

pub async fn platform_status(
    State(state): State<AppState>,
    Path(platform_id): Path<Uuid>,
) -> AppResult<Json<PlatformStatus>> {
    let repo = PassengerInfoRepository::new(&state.pool);
    Ok(Json(repo.platform_status(platform_id).await?))
}
