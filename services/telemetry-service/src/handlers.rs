use crate::{
    models::{HistoryQuery, PositionReportPoint, TrainTelemetrySummary},
    repository::TelemetryRepository,
    state::AppState,
};
use axum::{
    extract::{Path, Query, State},
    Json,
};
use cbtc_errors::AppResult;
use uuid::Uuid;

pub async fn history(
    State(state): State<AppState>,
    Path(train_id): Path<Uuid>,
    Query(q): Query<HistoryQuery>,
) -> AppResult<Json<Vec<PositionReportPoint>>> {
    let repo = TelemetryRepository::new(&state.pool);
    Ok(Json(
        repo.history(train_id, q.from, q.to, q.limit.clamp(1, 5000))
            .await?,
    ))
}

pub async fn latest(
    State(state): State<AppState>,
    Path(train_id): Path<Uuid>,
) -> AppResult<Json<Option<PositionReportPoint>>> {
    let repo = TelemetryRepository::new(&state.pool);
    Ok(Json(repo.latest(train_id).await?))
}

pub async fn summary(
    State(state): State<AppState>,
    Path(train_id): Path<Uuid>,
    Query(q): Query<HistoryQuery>,
) -> AppResult<Json<TrainTelemetrySummary>> {
    let repo = TelemetryRepository::new(&state.pool);
    Ok(Json(repo.summary(train_id, q.from, q.to).await?))
}
