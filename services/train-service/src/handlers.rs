use crate::{
    models::{CreateTrainRequest, PositionReportRequest, StateChangeRequest, TrainResponse},
    repository::{TrainRepository, TrainRow},
    state::AppState,
};
use axum::{
    extract::{Path, State},
    Json,
};
use cbtc_auth::{AuthenticatedUser, Role};
use cbtc_domain::{BlockId, MovementAuthorityId, TrainId};
use cbtc_errors::{AppError, AppResult};
use cbtc_events::{DomainEvent, EventEnvelope};
use uuid::Uuid;
use validator::Validate;

fn to_response(row: TrainRow) -> TrainResponse {
    TrainResponse {
        id: TrainId::from(row.id),
        identifier: row.identifier,
        operational_state: row.operational_state,
        current_block: row.current_block.map(BlockId::from),
        active_authority: row.active_authority.map(MovementAuthorityId::from),
        max_speed_kph: row.max_speed_kph,
        length_m: row.length_m,
        last_heartbeat: row.last_heartbeat,
    }
}

/// `POST /trains` — operator/admin only.
pub async fn create_train(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Json(req): Json<CreateTrainRequest>,
) -> AppResult<Json<TrainResponse>> {
    auth_user.require_role(Role::Operator)?;
    req.validate()
        .map_err(|e| AppError::validation(e.to_string()))?;

    let repo = TrainRepository::new(&state.pool);
    let row = repo
        .create(
            &req.identifier,
            req.max_speed_kph,
            req.length_m,
            req.initial_block.map(|b| b.as_uuid()),
        )
        .await?;

    tracing::info!(train_id = %row.id, identifier = %row.identifier, "train created");
    Ok(Json(to_response(row)))
}

/// `GET /trains`
pub async fn list_trains(State(state): State<AppState>) -> AppResult<Json<Vec<TrainResponse>>> {
    let repo = TrainRepository::new(&state.pool);
    let rows = repo.list_all().await?;
    Ok(Json(rows.into_iter().map(to_response).collect()))
}

/// `GET /trains/:id`
pub async fn get_train(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<TrainResponse>> {
    let repo = TrainRepository::new(&state.pool);
    let row = repo
        .find_by_id(id)
        .await?
        .ok_or_else(|| AppError::not_found("train", id.to_string()))?;
    Ok(Json(to_response(row)))
}

/// `POST /trains/:id/position-reports` — ingests live telemetry. This is
/// the highest-frequency endpoint in the platform (nominally one call
/// per train per second in the simulation), so it deliberately does the
/// minimum work needed: persist the report, update the train's current
/// block/heartbeat, and publish a `TrainPositionReported` event for
/// block-occupancy-service and the dashboard to react to. It does not
/// itself perform any safety evaluation — that's interlocking-service's
/// job, triggered by the event this handler publishes.
pub async fn record_position_report(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<PositionReportRequest>,
) -> AppResult<Json<TrainResponse>> {
    req.validate()
        .map_err(|e| AppError::validation(e.to_string()))?;

    let repo = TrainRepository::new(&state.pool);
    // Row must exist before we accept telemetry for it.
    let existing = repo
        .find_by_id(id)
        .await?
        .ok_or_else(|| AppError::not_found("train", id.to_string()))?;

    if existing.operational_state == cbtc_domain::TrainOperationalState::OutOfService {
        return Err(AppError::conflict(
            "cannot record telemetry for an out-of-service train",
        ));
    }

    let applied = repo
        .record_position_report(
            id,
            req.occupied_block.as_uuid(),
            req.offset_within_block_m,
            req.speed_kph,
            req.acceleration_mps2,
            req.client_timestamp,
        )
        .await?;

    if !applied {
        // Stale/out-of-order relative to a report already recorded for
        // this train (Phase 8) — heartbeat was still refreshed inside
        // the repository call, but the position itself is discarded
        // rather than silently corrupting current_block with older
        // data. No event is published: nothing about the train's
        // authoritative state actually changed.
        tracing::debug!(train_id = %id, client_timestamp = %req.client_timestamp, "discarded stale/out-of-order position report");
        let current = repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::not_found("train", id.to_string()))?;
        return Err(AppError::conflict(format!(
            "position report is stale or out-of-order for train {}; current state unchanged",
            current.identifier
        )));
    }

    let envelope = EventEnvelope::originate(
        "train-service",
        DomainEvent::TrainPositionReported {
            train_id: TrainId::from(id),
            block_id: req.occupied_block,
            offset_within_block_m: req.offset_within_block_m,
            speed_kph: req.speed_kph,
        },
    );
    if let Err(e) = state.mq.publish(&envelope).await {
        // Telemetry publish failures must not fail the ingest call itself
        // (the position was already durably persisted above) — they're
        // logged as warnings so downstream reactive consumers (occupancy,
        // dashboard) simply see a gap rather than the train losing its
        // recorded position.
        tracing::warn!(error = %e, train_id = %id, "failed to publish TrainPositionReported");
    }

    let updated = repo
        .find_by_id(id)
        .await?
        .ok_or_else(|| AppError::not_found("train", id.to_string()))?;
    Ok(Json(to_response(updated)))
}

/// `POST /trains/:id/heartbeat` — lightweight keep-alive distinct from a
/// full position report, used when a train has nothing new to report but
/// must still prove it hasn't lost communication.
pub async fn heartbeat(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<TrainResponse>> {
    let repo = TrainRepository::new(&state.pool);
    repo.touch_heartbeat(id).await?;
    let row = repo
        .find_by_id(id)
        .await?
        .ok_or_else(|| AppError::not_found("train", id.to_string()))?;
    Ok(Json(to_response(row)))
}

/// `PATCH /trains/:id/state` — operator-only manual state transition
/// (e.g. taking a train out of service, or clearing a degraded state
/// after maintenance). Automatic transitions (emergency braking on
/// safety violation, communication_lost on heartbeat timeout) are
/// applied directly by interlocking-service and the background monitor
/// respectively, not through this endpoint.
pub async fn change_state(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<Uuid>,
    Json(req): Json<StateChangeRequest>,
) -> AppResult<Json<TrainResponse>> {
    auth_user.require_role(Role::Operator)?;
    req.validate()
        .map_err(|e| AppError::validation(e.to_string()))?;

    let repo = TrainRepository::new(&state.pool);
    let existing = repo
        .find_by_id(id)
        .await?
        .ok_or_else(|| AppError::not_found("train", id.to_string()))?;

    repo.set_state(id, req.new_state).await?;

    let envelope = EventEnvelope::originate(
        "train-service",
        DomainEvent::TrainStateChanged {
            train_id: TrainId::from(id),
            previous_state: format!("{:?}", existing.operational_state),
            new_state: format!("{:?}", req.new_state),
        },
    );
    if let Err(e) = state.mq.publish(&envelope).await {
        tracing::warn!(error = %e, train_id = %id, "failed to publish TrainStateChanged");
    }

    let updated = repo
        .find_by_id(id)
        .await?
        .ok_or_else(|| AppError::not_found("train", id.to_string()))?;
    Ok(Json(to_response(updated)))
}
