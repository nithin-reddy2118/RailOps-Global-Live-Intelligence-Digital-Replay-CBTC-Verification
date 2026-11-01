use crate::{
    models::{OpenTicketRequest, TicketResponse, TicketStatus, UpdateTicketRequest},
    repository::TicketRepository,
    state::AppState,
};
use axum::{
    extract::{Path, State},
    Json,
};
use cbtc_auth::{AuthenticatedUser, Role};
use cbtc_errors::{AppError, AppResult};
use cbtc_events::{DomainEvent, EventEnvelope};
use uuid::Uuid;
use validator::Validate;

pub async fn open_ticket(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Json(req): Json<OpenTicketRequest>,
) -> AppResult<Json<TicketResponse>> {
    if !auth_user.has_role(Role::Maintainer)
        && !auth_user.has_role(Role::Admin)
        && !auth_user.has_role(Role::Operator)
    {
        return Err(AppError::forbidden(
            "requires maintainer, operator, or admin role",
        ));
    }
    req.validate()
        .map_err(|e| AppError::validation(e.to_string()))?;

    let repo = TicketRepository::new(&state.pool);
    let ticket = repo
        .open(
            &req.asset_description,
            req.related_block,
            req.related_switch,
            req.related_signal,
            auth_user.user_id.as_uuid(),
        )
        .await?;

    let envelope = EventEnvelope::originate(
        "maintenance-service",
        DomainEvent::MaintenanceTicketOpened {
            ticket_id: ticket.id,
            asset_description: ticket.asset_description.clone(),
        },
    );
    if let Err(e) = state.mq.publish(&envelope).await {
        tracing::warn!(error = %e, ticket_id = %ticket.id, "failed to publish MaintenanceTicketOpened");
    }

    Ok(Json(ticket))
}

pub async fn list_open(State(state): State<AppState>) -> AppResult<Json<Vec<TicketResponse>>> {
    let repo = TicketRepository::new(&state.pool);
    Ok(Json(repo.list_open().await?))
}

pub async fn list_all(State(state): State<AppState>) -> AppResult<Json<Vec<TicketResponse>>> {
    let repo = TicketRepository::new(&state.pool);
    Ok(Json(repo.list_all(200).await?))
}

pub async fn get_ticket(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<TicketResponse>> {
    let repo = TicketRepository::new(&state.pool);
    let ticket = repo
        .find_by_id(id)
        .await?
        .ok_or_else(|| AppError::not_found("maintenance_ticket", id.to_string()))?;
    Ok(Json(ticket))
}

pub async fn update_ticket(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateTicketRequest>,
) -> AppResult<Json<TicketResponse>> {
    if !auth_user.has_role(Role::Maintainer) && !auth_user.has_role(Role::Admin) {
        return Err(AppError::forbidden("requires maintainer or admin role"));
    }

    let repo = TicketRepository::new(&state.pool);
    let ticket = repo
        .update_status(id, req.status, req.notes.as_deref())
        .await?;

    if matches!(req.status, TicketStatus::Resolved | TicketStatus::Closed) {
        tracing::info!(ticket_id = %id, status = ?req.status, "maintenance ticket closed out");
    }

    Ok(Json(ticket))
}
