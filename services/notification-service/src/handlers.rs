use crate::{models::AlertResponse, repository::AlertRepository, state::AppState};
use axum::{
    extract::{Path, Query, State},
    Json,
};
use cbtc_auth::AuthenticatedUser;
use cbtc_errors::AppResult;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct RecentQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_limit() -> i64 {
    100
}

pub async fn list_unacknowledged(
    State(state): State<AppState>,
) -> AppResult<Json<Vec<AlertResponse>>> {
    let repo = AlertRepository::new(&state.pool);
    Ok(Json(repo.list_unacknowledged().await?))
}

pub async fn list_recent(
    State(state): State<AppState>,
    Query(q): Query<RecentQuery>,
) -> AppResult<Json<Vec<AlertResponse>>> {
    let repo = AlertRepository::new(&state.pool);
    Ok(Json(repo.list_recent(q.limit.clamp(1, 500)).await?))
}

pub async fn acknowledge(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<AlertResponse>> {
    let repo = AlertRepository::new(&state.pool);
    Ok(Json(
        repo.acknowledge(id, auth_user.user_id.as_uuid()).await?,
    ))
}
