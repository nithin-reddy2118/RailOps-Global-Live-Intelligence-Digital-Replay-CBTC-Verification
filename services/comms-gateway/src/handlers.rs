use crate::{models::PositionReportRequest, state::AppState};
use axum::{
    extract::{Path, State},
    Json,
};
use cbtc_auth::{AuthenticatedUser, Role};
use cbtc_domain::TrainId;
use cbtc_errors::{AppError, AppResult};
use cbtc_events::{DomainEvent, EventEnvelope};
use rand::Rng;
use std::time::Duration;
use uuid::Uuid;

/// Apply configured latency/drop/duplicate to a downstream forward.
/// Returns `Ok(true)` if the caller should proceed to forward (possibly
/// more than once), `Ok(false)` if the message should be silently
/// dropped (simulating a lost transmission — the caller still returns
/// 202 Accepted to the train, exactly as a real radio link would give
/// the train no synchronous confirmation of wayside receipt).
async fn apply_fault_injection(state: &AppState) -> bool {
    if !state.fault_injection.enabled {
        return true;
    }

    if state.fault_injection.latency_ms_max > 0 {
        let delay = {
            let mut rng = rand::thread_rng();
            let min = state
                .fault_injection
                .latency_ms_min
                .min(state.fault_injection.latency_ms_max);
            rng.gen_range(min..=state.fault_injection.latency_ms_max)
        };
        if delay > 0 {
            tokio::time::sleep(Duration::from_millis(delay)).await;
        }
    }

    let drop_roll: f64 = rand::thread_rng().gen();
    if drop_roll < state.fault_injection.packet_loss_probability {
        return false;
    }

    true
}

fn should_duplicate(state: &AppState) -> bool {
    if !state.fault_injection.enabled {
        return false;
    }
    let roll: f64 = rand::thread_rng().gen();
    roll < state.fault_injection.duplicate_message_probability
}

/// `POST /comms/trains/:id/position-reports` — the simulated wireless
/// uplink a train uses to reach wayside/control infrastructure. Applies
/// blackout simulation and fault injection before relaying to
/// train-service's real ingestion endpoint.
pub async fn relay_position_report(
    State(state): State<AppState>,
    Path(train_id): Path<Uuid>,
    Json(req): Json<PositionReportRequest>,
) -> AppResult<axum::http::StatusCode> {
    if state.blackouts.is_in_blackout(train_id) {
        tracing::debug!(train_id = %train_id, "position report dropped: train is in simulated blackout");
        return Ok(axum::http::StatusCode::ACCEPTED);
    }

    if !apply_fault_injection(&state).await {
        tracing::debug!(train_id = %train_id, "position report dropped by fault injection");
        return Ok(axum::http::StatusCode::ACCEPTED);
    }

    forward_position_report(&state, train_id, &req).await?;

    if should_duplicate(&state) {
        tracing::debug!(train_id = %train_id, "duplicating position report per fault injection");
        let _ = forward_position_report(&state, train_id, &req).await;
    }

    Ok(axum::http::StatusCode::ACCEPTED)
}

async fn forward_position_report(
    state: &AppState,
    train_id: Uuid,
    req: &PositionReportRequest,
) -> AppResult<()> {
    let url = format!(
        "{}/trains/{}/position-reports",
        state.train_service_base_url, train_id
    );
    let response =
        state
            .http
            .post(&url)
            .json(req)
            .send()
            .await
            .map_err(|e| AppError::UpstreamService {
                service: "train-service",
                status: e.status().map(|s| s.as_u16()).unwrap_or(0),
            })?;

    if !response.status().is_success() {
        return Err(AppError::UpstreamService {
            service: "train-service",
            status: response.status().as_u16(),
        });
    }
    Ok(())
}

/// `POST /comms/trains/:id/heartbeat`
pub async fn relay_heartbeat(
    State(state): State<AppState>,
    Path(train_id): Path<Uuid>,
) -> AppResult<axum::http::StatusCode> {
    if state.blackouts.is_in_blackout(train_id) {
        return Ok(axum::http::StatusCode::ACCEPTED);
    }
    if !apply_fault_injection(&state).await {
        return Ok(axum::http::StatusCode::ACCEPTED);
    }

    let url = format!(
        "{}/trains/{}/heartbeat",
        state.train_service_base_url, train_id
    );
    let response = state
        .http
        .post(&url)
        .send()
        .await
        .map_err(|e| AppError::UpstreamService {
            service: "train-service",
            status: e.status().map(|s| s.as_u16()).unwrap_or(0),
        })?;

    if !response.status().is_success() {
        return Err(AppError::UpstreamService {
            service: "train-service",
            status: response.status().as_u16(),
        });
    }
    Ok(axum::http::StatusCode::ACCEPTED)
}

/// `POST /comms/trains/:id/blackout/start` — operator/admin-only demo
/// control: every subsequent message from this train is dropped at the
/// gateway (never reaching train-service) until blackout is ended. This
/// is what actually produces a `TrainHeartbeatMissed` /
/// `CommunicationLost` transition in train-service after
/// `Train::HEARTBEAT_TIMEOUT_SECS`.
pub async fn start_blackout(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(train_id): Path<Uuid>,
) -> AppResult<axum::http::StatusCode> {
    auth_user.require_role(Role::Operator)?;
    state.blackouts.set(train_id, true);

    let envelope = EventEnvelope::originate(
        "comms-gateway",
        DomainEvent::CommunicationBlackoutStarted {
            train_id: TrainId::from(train_id),
        },
    );
    let _ = state.mq.publish(&envelope).await;

    tracing::warn!(train_id = %train_id, "simulated communication blackout started");
    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// `POST /comms/trains/:id/blackout/stop`
pub async fn stop_blackout(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(train_id): Path<Uuid>,
) -> AppResult<axum::http::StatusCode> {
    auth_user.require_role(Role::Operator)?;
    state.blackouts.set(train_id, false);

    let envelope = EventEnvelope::originate(
        "comms-gateway",
        DomainEvent::CommunicationRestored {
            train_id: TrainId::from(train_id),
        },
    );
    let _ = state.mq.publish(&envelope).await;

    tracing::info!(train_id = %train_id, "simulated communication blackout ended");
    Ok(axum::http::StatusCode::NO_CONTENT)
}
