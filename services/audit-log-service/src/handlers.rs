use crate::{models::AuditEntryResponse, repository::AuditRepository, state::AppState};
use axum::{
    extract::{Path, Query, State},
    Json,
};
use cbtc_auth::{AuthenticatedUser, Role};
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

/// `GET /audit/recent?limit=N` — admin-only, since the audit trail can
/// reveal operational and personal (login) details.
pub async fn recent(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Query(q): Query<RecentQuery>,
) -> AppResult<Json<Vec<AuditEntryResponse>>> {
    auth_user.require_role(Role::Admin)?;
    let repo = AuditRepository::new(&state.pool);
    Ok(Json(repo.recent(q.limit.clamp(1, 1000)).await?))
}

/// `GET /audit/causality/:id` — trace every audit row sharing a causal
/// chain, e.g. to reconstruct exactly what happened following a single
/// train's position report through occupancy, interlocking, and
/// authority services.
pub async fn by_causality(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(causality_id): Path<Uuid>,
) -> AppResult<Json<Vec<AuditEntryResponse>>> {
    auth_user.require_role(Role::Admin)?;
    let repo = AuditRepository::new(&state.pool);
    Ok(Json(repo.by_causality(causality_id).await?))
}
