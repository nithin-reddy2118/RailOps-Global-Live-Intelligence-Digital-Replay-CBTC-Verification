use crate::{
    models::{BlockResponse, ManualOccupancyRequest},
    repository::{BlockRepository, BlockRow},
    state::AppState,
};
use axum::{
    extract::{Path, State},
    Json,
};
use cbtc_auth::{AuthenticatedUser, Role};
use cbtc_domain::{BlockId, TrainId};
use cbtc_errors::{AppError, AppResult};
use cbtc_events::{DomainEvent, EventEnvelope};
use uuid::Uuid;
use validator::Validate;

fn to_response(row: BlockRow) -> BlockResponse {
    BlockResponse {
        id: BlockId::from(row.id),
        name: row.name,
        length_m: row.length_m,
        occupancy: row.occupancy,
        occupied_by: row.occupied_by.map(TrainId::from),
        last_updated: row.last_updated,
        max_speed_kph: row.max_speed_kph,
        bidirectional: row.bidirectional,
    }
}

pub async fn list_blocks(State(state): State<AppState>) -> AppResult<Json<Vec<BlockResponse>>> {
    let repo = BlockRepository::new(&state.pool);
    let rows = repo.list_all().await?;
    Ok(Json(rows.into_iter().map(to_response).collect()))
}

pub async fn get_block(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<BlockResponse>> {
    let repo = BlockRepository::new(&state.pool);
    let row = repo
        .find_by_id(id)
        .await?
        .ok_or_else(|| AppError::not_found("block", id.to_string()))?;
    Ok(Json(to_response(row)))
}

/// `PATCH /blocks/:id/occupancy` — manual override, restricted to
/// operators/maintainers. Used to take a block `out_of_service` for
/// maintenance, or to manually clear an `ambiguous` state once an
/// operator has visually/physically confirmed the block is actually
/// clear (the one case where a human overrides automated detection).
pub async fn set_manual_occupancy(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<Uuid>,
    Json(req): Json<ManualOccupancyRequest>,
) -> AppResult<Json<BlockResponse>> {
    if !auth_user.has_role(Role::Operator)
        && !auth_user.has_role(Role::Maintainer)
        && !auth_user.has_role(Role::Admin)
    {
        return Err(AppError::forbidden(
            "requires operator, maintainer, or admin role",
        ));
    }
    req.validate()
        .map_err(|e| AppError::validation(e.to_string()))?;

    let repo = BlockRepository::new(&state.pool);
    let existing = repo
        .find_by_id(id)
        .await?
        .ok_or_else(|| AppError::not_found("block", id.to_string()))?;

    repo.set_manual_occupancy(id, req.occupancy).await?;

    let envelope = EventEnvelope::originate(
        "block-occupancy-service",
        DomainEvent::BlockOccupancyChanged {
            block_id: BlockId::from(id),
            previous_state: format!("{:?}", existing.occupancy),
            new_state: format!("{:?}", req.occupancy),
            train_id: None,
        },
    );
    if let Err(e) = state.mq.publish(&envelope).await {
        tracing::warn!(error = %e, block_id = %id, "failed to publish BlockOccupancyChanged");
    }

    tracing::info!(block_id = %id, reason = %req.reason, new_state = ?req.occupancy, "manual occupancy override applied");

    let updated = repo
        .find_by_id(id)
        .await?
        .ok_or_else(|| AppError::not_found("block", id.to_string()))?;
    Ok(Json(to_response(updated)))
}
