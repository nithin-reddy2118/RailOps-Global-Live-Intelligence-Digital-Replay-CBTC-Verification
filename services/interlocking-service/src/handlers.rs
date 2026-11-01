use crate::{
    ma_client,
    models::{RequestAuthorityRequest, RequestAuthorityResponse, SwitchCommandRequest},
    repository::InterlockingRepository,
    safety::{check_path_conflicts, check_switch_states},
    state::AppState,
};
use axum::{
    extract::{Path, State},
    Json,
};
use cbtc_auth::{AuthenticatedUser, Role};
use cbtc_domain::{BlockId, SwitchId, SwitchPosition};
use cbtc_errors::{AppError, AppResult};
use cbtc_events::{DomainEvent, EventEnvelope};
use uuid::Uuid;
use validator::Validate;

/// `POST /interlocking/request-authority` Ã¢â‚¬â€ the platform's central
/// safety decision point. Given a train and a destination block, this:
///   1. Computes a candidate path via the topology cache.
///   2. Re-checks occupancy, switch state, and authority overlap live
///      against the database (never the cache, for these three checks Ã¢â‚¬â€
///      see `safety.rs` for why).
///   3. If clear, calls movement-authority-service to actually grant.
///      Any conflict results in a structured rejection rather than an HTTP
///      error, since "no safe path right now" is an expected, common outcome
///      during normal operation, not a fault.
pub async fn request_authority(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Json(req): Json<RequestAuthorityRequest>,
) -> AppResult<Json<RequestAuthorityResponse>> {
    auth_user.require_role(Role::Operator)?;
    req.validate()
        .map_err(|e| AppError::validation(e.to_string()))?;

    let repo = InterlockingRepository::new(&state.pool);

    let current_block = repo
        .get_train_current_block(req.train_id.as_uuid())
        .await?
        .ok_or_else(|| {
            AppError::conflict("train has no known current block; cannot compute a route")
        })?;

    let path = state
        .topology
        .find_path(BlockId::from(current_block), req.destination_block);

    let Some(path) = path else {
        return Ok(Json(RequestAuthorityResponse::NoPathFound));
    };

    let path_uuids: Vec<Uuid> = path.iter().map(|b| b.as_uuid()).collect();

    let occupancy = repo.occupancy_for_blocks(&path_uuids).await?;
    let other_authority_blocks = repo
        .blocks_under_other_active_authorities(req.train_id.as_uuid())
        .await?;
    let gating_switches = repo.switch_positions_gating_path(&path_uuids).await?;

    let mut conflicts = check_path_conflicts(
        BlockId::from(current_block),
        &path,
        &occupancy,
        &other_authority_blocks,
    );
    conflicts.extend(check_switch_states(&gating_switches));

    if !conflicts.is_empty() {
        let reasons: Vec<String> = conflicts.iter().map(|c| c.to_string()).collect();
        tracing::info!(train_id = %req.train_id, reasons = ?reasons, "authority request rejected");
        return Ok(Json(RequestAuthorityResponse::Rejected { reasons }));
    }

    let terminal_block_length_m: f64 =
        sqlx::query_scalar("SELECT length_m FROM signaling.blocks WHERE id = $1")
            .bind(path_uuids.last().copied().unwrap())
            .fetch_one(&state.pool)
            .await?;

    let granted = ma_client::grant_authority(
        &state.http,
        &state.peers,
        req.train_id,
        path.clone(),
        terminal_block_length_m,
        req.max_authorized_speed_kph,
        req.ttl_secs,
    )
    .await?;

    let granted = match granted {
        ma_client::GrantOutcome::Granted(g) => g,
        ma_client::GrantOutcome::Conflict(detail) => {
            // The read-side check above passed, but movement-authority-
            // service's atomic grant found a genuine conflict against a
            // concurrently-committed authority for another train Ã¢â‚¬â€ this
            // is exactly the race Phase 6 exists to close. Surface it
            // as a structured rejection, the same as a conflict caught
            // by the fast-path check earlier in this handler.
            tracing::warn!(train_id = %req.train_id, detail, "authority request lost a concurrent race at the atomic grant boundary");
            return Ok(Json(RequestAuthorityResponse::Rejected {
                reasons: vec![format!(
                    "lost race to a concurrently granted authority: {detail}"
                )],
            }));
        }
    };

    let envelope = EventEnvelope::originate(
        "interlocking-service",
        DomainEvent::MovementAuthorityRequested {
            train_id: req.train_id,
            requested_blocks: path.clone(),
        },
    );
    let _ = state.mq.publish(&envelope).await;

    tracing::info!(train_id = %req.train_id, authority_id = %granted.id, path_len = path.len(), "authority granted via interlocking");

    Ok(Json(RequestAuthorityResponse::Granted {
        authority_id: granted.id,
        block_sequence: path,
    }))
}

/// `POST /interlocking/switches/:id/command` Ã¢â‚¬â€ commands a switch to a
/// new position. Rejects `InTransition`/`Failed` as target positions
/// (those are only ever *observed* states, never commanded), and
/// rejects the command outright if the switch is currently locked by an
/// active route through it.
pub async fn command_switch(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(switch_id): Path<Uuid>,
    Json(req): Json<SwitchCommandRequest>,
) -> AppResult<axum::http::StatusCode> {
    auth_user.require_role(Role::Operator)?;

    if !matches!(
        req.position,
        SwitchPosition::Normal | SwitchPosition::Reverse
    ) {
        return Err(AppError::validation(
            "switches may only be commanded to Normal or Reverse",
        ));
    }

    let repo = InterlockingRepository::new(&state.pool);
    let (current_position, locked) = repo
        .get_switch_position(switch_id)
        .await?
        .ok_or_else(|| AppError::not_found("switch", switch_id.to_string()))?;

    if locked {
        return Err(AppError::conflict(
            "switch is locked by an active route and cannot be commanded",
        ));
    }

    repo.set_switch_position(switch_id, req.position).await?;

    let envelope = EventEnvelope::originate(
        "interlocking-service",
        DomainEvent::SwitchPositionChanged {
            switch_id: SwitchId::from(switch_id),
            previous_position: format!("{current_position:?}"),
            new_position: format!("{:?}", req.position),
        },
    );
    if let Err(e) = state.mq.publish(&envelope).await {
        tracing::warn!(error = %e, switch_id = %switch_id, "failed to publish SwitchPositionChanged");
    }

    state
        .topology
        .update_switch_position(SwitchId::from(switch_id), req.position);

    tracing::info!(switch_id = %switch_id, new_position = ?req.position, "switch commanded");
    Ok(axum::http::StatusCode::NO_CONTENT)
}
