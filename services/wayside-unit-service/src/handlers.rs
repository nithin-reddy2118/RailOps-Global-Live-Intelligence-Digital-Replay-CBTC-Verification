use crate::{repository::WaysideRepository, state::AppState};
use axum::{
    extract::{Path, State},
    Json,
};
use cbtc_auth::{AuthenticatedUser, Role};
use cbtc_domain::{SignalAspect, SignalId};
use cbtc_errors::{AppError, AppResult};
use cbtc_events::{DomainEvent, EventEnvelope};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema)]
pub struct SignalStateResponse {
    pub signal_id: Uuid,
    #[schema(value_type = String)]
    pub aspect: SignalAspect,
}

pub async fn get_signal(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<SignalStateResponse>> {
    let repo = WaysideRepository::new(&state.pool);
    let (_, aspect) = repo
        .get_signal(id)
        .await?
        .ok_or_else(|| AppError::not_found("signal", id.to_string()))?;
    Ok(Json(SignalStateResponse {
        signal_id: id,
        aspect,
    }))
}

/// `POST /wayside/signals/:id/inject-fault` — maintainer/admin only.
/// Forces a signal into the `Failed` aspect to exercise degraded-mode
/// handling elsewhere in the platform (interlocking-service treats
/// `Failed` as equivalent to `Red`). This is a manual failure-simulation
/// tool for demos, distinct from the automatic aspect recomputation
/// driven by real occupancy changes in `mq_consumer.rs`.
pub async fn inject_fault(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<Uuid>,
) -> AppResult<axum::http::StatusCode> {
    if !auth_user.has_role(Role::Maintainer) && !auth_user.has_role(Role::Admin) {
        return Err(AppError::forbidden("requires maintainer or admin role"));
    }

    let repo = WaysideRepository::new(&state.pool);
    let (_, previous_aspect) = repo
        .get_signal(id)
        .await?
        .ok_or_else(|| AppError::not_found("signal", id.to_string()))?;

    repo.set_aspect(id, SignalAspect::Failed).await?;

    let envelope = EventEnvelope::originate(
        "wayside-unit-service",
        DomainEvent::SignalAspectChanged {
            signal_id: SignalId::from(id),
            previous_aspect: format!("{previous_aspect:?}"),
            new_aspect: "Failed".to_string(),
        },
    );
    let _ = state.mq.publish(&envelope).await;

    tracing::warn!(signal_id = %id, "signal fault manually injected");
    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// `POST /wayside/signals/:id/clear-fault` — restores a signal to Red
/// (the always-safe default) rather than directly to Green; the next
/// real occupancy change will recompute its true aspect. Never restores
/// straight to Green, since that would bypass the fail-safe.
pub async fn clear_fault(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<Uuid>,
) -> AppResult<axum::http::StatusCode> {
    if !auth_user.has_role(Role::Maintainer) && !auth_user.has_role(Role::Admin) {
        return Err(AppError::forbidden("requires maintainer or admin role"));
    }

    let repo = WaysideRepository::new(&state.pool);
    repo.get_signal(id)
        .await?
        .ok_or_else(|| AppError::not_found("signal", id.to_string()))?;
    repo.set_aspect(id, SignalAspect::Red).await?;

    let envelope = EventEnvelope::originate(
        "wayside-unit-service",
        DomainEvent::SignalAspectChanged {
            signal_id: SignalId::from(id),
            previous_aspect: "Failed".to_string(),
            new_aspect: "Red".to_string(),
        },
    );
    let _ = state.mq.publish(&envelope).await;

    tracing::info!(signal_id = %id, "signal fault cleared; aspect reset to Red pending recomputation");
    Ok(axum::http::StatusCode::NO_CONTENT)
}
