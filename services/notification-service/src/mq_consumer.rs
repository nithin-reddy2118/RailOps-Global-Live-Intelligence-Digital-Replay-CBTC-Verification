use crate::{models::AlertSeverity, repository::AlertRepository, state::AppState};
use cbtc_errors::AppError;
use cbtc_events::{DomainEvent, EventEnvelope};

pub async fn run(state: AppState) -> Result<(), AppError> {
    let handler_state = state.clone();
    state
        .mq
        .consume(
            "notification-service.alert-events",
            &[
                "cbtc.safety.emergency_brake",
                "cbtc.safety.collision_risk",
                "cbtc.safety.block_occupancy_conflict",
                "cbtc.switch.fault_detected",
                "cbtc.train.heartbeat_missed",
                "cbtc.maintenance.ticket_opened",
                "cbtc.comms.blackout_started",
            ],
            move |envelope| {
                let state = handler_state.clone();
                async move { handle_event(&state, envelope).await }
            },
        )
        .await
}

async fn handle_event(
    state: &AppState,
    envelope: EventEnvelope<DomainEvent>,
) -> Result<(), AppError> {
    let repo = AlertRepository::new(&state.pool);

    let (severity, title, description) = match &envelope.payload {
        DomainEvent::EmergencyBrakeTriggered { train_id, reason } => (
            AlertSeverity::Critical,
            format!("Emergency brake — train {train_id}"),
            format!("Train {train_id} triggered emergency braking: {reason}"),
        ),
        DomainEvent::CollisionRiskDetected { train_ids, block_id } => (
            AlertSeverity::Critical,
            format!("Collision risk in block {block_id}"),
            format!("Trains {train_ids:?} both associated with block {block_id}"),
        ),
        DomainEvent::BlockOccupancyConflict { block_id, incumbent_train_id, reporting_train_id } => (
            AlertSeverity::Critical,
            format!("Occupancy conflict — block {block_id}"),
            format!(
                "Train {reporting_train_id} reported occupying block {block_id}, already held by train {incumbent_train_id}. Incumbent occupancy was NOT overwritten."
            ),
        ),
        DomainEvent::SwitchFaultDetected { switch_id, description } => (
            AlertSeverity::Critical,
            format!("Switch fault — {switch_id}"),
            description.clone(),
        ),
        DomainEvent::TrainHeartbeatMissed { train_id, seconds_since_last_heartbeat } => (
            AlertSeverity::Warning,
            format!("Heartbeat missed — train {train_id}"),
            format!("No telemetry received for {seconds_since_last_heartbeat}s"),
        ),
        DomainEvent::MaintenanceTicketOpened { ticket_id, asset_description } => (
            AlertSeverity::Info,
            "Maintenance ticket opened".to_string(),
            format!("Ticket {ticket_id}: {asset_description}"),
        ),
        DomainEvent::CommunicationBlackoutStarted { train_id } => (
            AlertSeverity::Warning,
            format!("Communication blackout — train {train_id}"),
            "Simulated communication blackout in effect".to_string(),
        ),
        _ => return Ok(()),
    };

    repo.create(
        severity,
        &title,
        &description,
        &envelope.origin_service,
        Some(envelope.event_id),
    )
    .await?;
    tracing::info!(title = %title, severity = ?severity, "alert created");
    Ok(())
}
