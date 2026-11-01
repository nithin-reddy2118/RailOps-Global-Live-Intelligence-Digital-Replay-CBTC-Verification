use crate::{
    models::{AuthorityResponse, GrantAuthorityRequest, RevokeAuthorityRequest},
    repository::{AuthorityRepository, AuthorityRow},
    state::AppState,
};
use axum::{
    extract::{Path, State},
    Json,
};
use cbtc_auth::{AuthenticatedUser, Role};
use cbtc_domain::{AuthorityStatus, BlockId, MovementAuthorityId, TrainId};
use cbtc_errors::{AppError, AppResult};
use cbtc_events::{DomainEvent, EventEnvelope};
use uuid::Uuid;
use validator::Validate;

fn to_response(row: AuthorityRow) -> AuthorityResponse {
    AuthorityResponse {
        id: MovementAuthorityId::from(row.id),
        train_id: TrainId::from(row.train_id),
        block_sequence: row.block_sequence.into_iter().map(BlockId::from).collect(),
        limit_of_movement_m: row.limit_of_movement_m,
        max_authorized_speed_kph: row.max_authorized_speed_kph,
        status: row.status,
        issued_at: row.issued_at,
        expires_at: row.expires_at,
    }
}

/// `POST /authorities` — grants a new movement authority. Callable by
/// operators directly (manual authority issuance from the control room)
/// or by interlocking-service acting on an operator's/automatic route
/// request; either way the caller is responsible for having already
/// validated the route is conflict-free. This service enforces only the
/// structural invariant (one live authority per train), not the
/// signaling-safety invariant (that's interlocking-service's domain).
pub async fn grant_authority(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Json(req): Json<GrantAuthorityRequest>,
) -> AppResult<Json<AuthorityResponse>> {
    auth_user.require_role(Role::Operator)?;
    req.validate()
        .map_err(|e| AppError::validation(e.to_string()))?;

    let repo = AuthorityRepository::new(&state.pool);
    let row = match repo
        .grant_if_no_conflict(
            req.train_id.as_uuid(),
            req.block_sequence.iter().map(|b| b.as_uuid()).collect(),
            req.limit_of_movement_m,
            req.max_authorized_speed_kph,
            req.ttl_secs,
        )
        .await?
    {
        Ok(row) => row,
        Err(conflicting_blocks) => {
            return Err(AppError::conflict(format!(
                "requested route overlaps another train's live authority in block(s): {conflicting_blocks:?}"
            )));
        }
    };

    let envelope = EventEnvelope::originate(
        "movement-authority-service",
        DomainEvent::MovementAuthorityGranted {
            authority_id: MovementAuthorityId::from(row.id),
            train_id: req.train_id,
            limit_of_movement_m: row.limit_of_movement_m,
        },
    );
    if let Err(e) = state.mq.publish(&envelope).await {
        tracing::warn!(error = %e, authority_id = %row.id, "failed to publish MovementAuthorityGranted");
    }

    tracing::info!(authority_id = %row.id, train_id = %req.train_id, "movement authority granted");
    Ok(Json(to_response(row)))
}

pub async fn get_authority(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<AuthorityResponse>> {
    let repo = AuthorityRepository::new(&state.pool);
    let row = repo
        .find_by_id(id)
        .await?
        .ok_or_else(|| AppError::not_found("movement_authority", id.to_string()))?;
    Ok(Json(to_response(row)))
}

pub async fn get_active_for_train(
    State(state): State<AppState>,
    Path(train_id): Path<Uuid>,
) -> AppResult<Json<Option<AuthorityResponse>>> {
    let repo = AuthorityRepository::new(&state.pool);
    let row = repo.find_active_for_train(train_id).await?;
    Ok(Json(row.map(to_response)))
}

pub async fn list_authorities(
    State(state): State<AppState>,
) -> AppResult<Json<Vec<AuthorityResponse>>> {
    let repo = AuthorityRepository::new(&state.pool);
    let rows = repo.list_all().await?;
    Ok(Json(rows.into_iter().map(to_response).collect()))
}

/// `POST /authorities/:id/revoke` — used by interlocking-service when a
/// safety condition invalidates a previously-granted authority (e.g. a
/// switch fault appears within its block sequence), and by operators to
/// manually pull an authority back.
pub async fn revoke_authority(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<Uuid>,
    Json(req): Json<RevokeAuthorityRequest>,
) -> AppResult<Json<AuthorityResponse>> {
    auth_user.require_role(Role::Operator)?;
    req.validate()
        .map_err(|e| AppError::validation(e.to_string()))?;

    let repo = AuthorityRepository::new(&state.pool);
    let existing = repo
        .find_by_id(id)
        .await?
        .ok_or_else(|| AppError::not_found("movement_authority", id.to_string()))?;

    if !matches!(
        existing.status,
        AuthorityStatus::Granted | AuthorityStatus::Active
    ) {
        return Err(AppError::conflict("authority is not in a revocable state"));
    }

    repo.set_status(id, AuthorityStatus::Revoked).await?;

    let envelope = EventEnvelope::originate(
        "movement-authority-service",
        DomainEvent::MovementAuthorityRevoked {
            authority_id: MovementAuthorityId::from(id),
            train_id: TrainId::from(existing.train_id),
            reason: req.reason.clone(),
        },
    );
    if let Err(e) = state.mq.publish(&envelope).await {
        tracing::warn!(error = %e, authority_id = %id, "failed to publish MovementAuthorityRevoked");
    }

    tracing::warn!(authority_id = %id, reason = %req.reason, "movement authority revoked");

    let updated = repo
        .find_by_id(id)
        .await?
        .ok_or_else(|| AppError::not_found("movement_authority", id.to_string()))?;
    Ok(Json(to_response(updated)))
}
