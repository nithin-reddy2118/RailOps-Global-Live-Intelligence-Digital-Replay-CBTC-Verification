//! Background heartbeat monitor. Runs as a spawned tokio task alongside
//! the HTTP server, polling for trains that have gone quiet longer than
//! `Train::HEARTBEAT_TIMEOUT_SECS` and transitioning them to
//! `CommunicationLost`, publishing both `TrainHeartbeatMissed` and
//! `TrainStateChanged` so interlocking-service can react (a train with
//! no communication must not be treated as safely tracked, even if its
//! last known block occupancy hasn't changed).

use crate::{repository::TrainRepository, state::AppState};
use cbtc_domain::{Train, TrainId, TrainOperationalState};
use cbtc_events::{DomainEvent, EventEnvelope};
use std::time::Duration;

pub async fn run(state: AppState) {
    let mut interval = tokio::time::interval(Duration::from_secs(2));
    loop {
        interval.tick().await;
        if let Err(e) = check_once(&state).await {
            tracing::warn!(error = %e, "heartbeat monitor iteration failed");
        }
    }
}

async fn check_once(state: &AppState) -> cbtc_errors::AppResult<()> {
    let repo = TrainRepository::new(&state.pool);
    let stale = repo.list_newly_stale(Train::HEARTBEAT_TIMEOUT_SECS).await?;

    for row in stale {
        let seconds_since = (chrono::Utc::now() - row.last_heartbeat).num_seconds();

        tracing::warn!(train_id = %row.id, seconds_since_last_heartbeat = seconds_since, "train heartbeat missed");

        let missed_envelope = EventEnvelope::originate(
            "train-service",
            DomainEvent::TrainHeartbeatMissed {
                train_id: TrainId::from(row.id),
                seconds_since_last_heartbeat: seconds_since,
            },
        );
        let _ = state.mq.publish(&missed_envelope).await;

        repo.set_state(row.id, TrainOperationalState::CommunicationLost)
            .await?;

        let state_envelope = EventEnvelope::caused_by(
            "train-service",
            &missed_envelope,
            DomainEvent::TrainStateChanged {
                train_id: TrainId::from(row.id),
                previous_state: format!("{:?}", row.operational_state),
                new_state: format!("{:?}", TrainOperationalState::CommunicationLost),
            },
        );
        let _ = state.mq.publish(&state_envelope).await;
    }

    Ok(())
}
