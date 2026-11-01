//! Reacts to platform-wide safety-relevant events by revoking movement
//! authorities that a new development has made unsafe, and by keeping
//! the topology cache's switch-position view consistent across every
//! running replica of interlocking-service (not just the one that
//! issued the command).

use crate::{ma_client, state::AppState};
use cbtc_domain::{SwitchId, SwitchPosition};
use cbtc_errors::AppError;
use cbtc_events::{DomainEvent, EventEnvelope};

pub async fn run(state: AppState) -> Result<(), AppError> {
    let handler_state = state.clone();
    state
        .mq
        .consume(
            "interlocking-service.safety-events",
            &[
                "cbtc.switch.position_changed",
                "cbtc.switch.fault_detected",
                "cbtc.train.heartbeat_missed",
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
    match &envelope.payload {
        DomainEvent::SwitchPositionChanged {
            switch_id,
            new_position,
            ..
        } => {
            if let Some(position) = parse_switch_position(new_position) {
                state.topology.update_switch_position(*switch_id, position);
            }
        }

        DomainEvent::SwitchFaultDetected {
            switch_id,
            description,
        } => {
            tracing::error!(switch_id = %switch_id, description, "switch fault detected — revoking authorities through affected routes");
            state
                .topology
                .update_switch_position(*switch_id, SwitchPosition::Failed);
            revoke_authorities_touching_switch(state, *switch_id).await?;
        }

        DomainEvent::TrainHeartbeatMissed {
            train_id,
            seconds_since_last_heartbeat,
        } => {
            tracing::warn!(
                train_id = %train_id,
                seconds_since_last_heartbeat,
                "train heartbeat missed — revoking its active authority as a precaution"
            );
            revoke_active_authority_for_train(state, *train_id).await;

            let brake_envelope = EventEnvelope::caused_by(
                "interlocking-service",
                &envelope,
                DomainEvent::EmergencyBrakeTriggered {
                    train_id: *train_id,
                    reason: "communication lost".to_string(),
                },
            );
            let _ = state.mq.publish(&brake_envelope).await;
        }

        _ => {}
    }
    Ok(())
}

fn parse_switch_position(s: &str) -> Option<SwitchPosition> {
    match s {
        "Normal" => Some(SwitchPosition::Normal),
        "Reverse" => Some(SwitchPosition::Reverse),
        "InTransition" => Some(SwitchPosition::InTransition),
        "Failed" => Some(SwitchPosition::Failed),
        _ => None,
    }
}

async fn revoke_authorities_touching_switch(
    state: &AppState,
    switch_id: SwitchId,
) -> Result<(), AppError> {
    let affected_train_ids: Vec<(uuid::Uuid, uuid::Uuid)> = sqlx::query_as(
        r#"
        SELECT ma.id, ma.train_id
        FROM train.movement_authorities ma
        JOIN signaling.block_adjacency adj ON adj.via_switch = $1
        WHERE ma.status IN ('granted', 'active')
          AND (adj.from_block = ANY(ma.block_sequence) OR adj.to_block = ANY(ma.block_sequence))
        "#,
    )
    .bind(switch_id.as_uuid())
    .fetch_all(&state.pool)
    .await?;

    for (authority_id, _train_id) in affected_train_ids {
        if let Err(e) = ma_client::revoke_authority(
            &state.http,
            &state.peers,
            cbtc_domain::MovementAuthorityId::from(authority_id),
            "switch fault detected on route",
        )
        .await
        {
            tracing::warn!(error = %e, authority_id = %authority_id, "failed to revoke authority after switch fault");
        }
    }

    Ok(())
}

async fn revoke_active_authority_for_train(state: &AppState, train_id: cbtc_domain::TrainId) {
    let active: Option<(uuid::Uuid,)> = sqlx::query_as(
        "SELECT id FROM train.movement_authorities WHERE train_id = $1 AND status IN ('granted', 'active') LIMIT 1",
    )
    .bind(train_id.as_uuid())
    .fetch_optional(&state.pool)
    .await
    .ok()
    .flatten();

    if let Some((authority_id,)) = active {
        if let Err(e) = ma_client::revoke_authority(
            &state.http,
            &state.peers,
            cbtc_domain::MovementAuthorityId::from(authority_id),
            "train communication lost",
        )
        .await
        {
            tracing::warn!(error = %e, authority_id = %authority_id, "failed to revoke authority after heartbeat loss");
        }
    }
}
