//! Subscribes to the wildcard pattern `cbtc.#` (every event published on
//! the exchange) and writes one audit row per event. This is the only
//! service in the platform that binds a queue to the full wildcard —
//! every other consumer binds narrowly to the specific routing keys it
//! actually acts on.

use crate::{repository::AuditRepository, state::AppState};
use cbtc_errors::AppError;
use cbtc_events::{DomainEvent, EventEnvelope};

pub async fn run(state: AppState) -> Result<(), AppError> {
    let handler_state = state.clone();
    state
        .mq
        .consume(
            "audit-log-service.all-events",
            &["cbtc.#"],
            move |envelope| {
                let state = handler_state.clone();
                async move { persist(&state, envelope).await }
            },
        )
        .await
}

async fn persist(state: &AppState, envelope: EventEnvelope<DomainEvent>) -> Result<(), AppError> {
    let repo = AuditRepository::new(&state.pool);
    let action = envelope.payload.routing_key();
    let details = serde_json::to_value(&envelope.payload)?;

    repo.insert(
        envelope.event_id,
        envelope.causality_id,
        &envelope.origin_service,
        &action,
        details,
    )
    .await?;

    Ok(())
}
