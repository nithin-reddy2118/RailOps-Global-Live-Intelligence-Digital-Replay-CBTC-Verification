use crate::{repository::AuthorityRepository, state::AppState};
use cbtc_domain::{AuthorityStatus, MovementAuthorityId, TrainId};
use cbtc_events::{DomainEvent, EventEnvelope};
use std::time::Duration;

pub async fn run(state: AppState) {
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    loop {
        interval.tick().await;
        if let Err(e) = sweep_once(&state).await {
            tracing::warn!(error = %e, "authority expiry sweep failed");
        }
    }
}

async fn sweep_once(state: &AppState) -> cbtc_errors::AppResult<()> {
    let repo = AuthorityRepository::new(&state.pool);
    let expired = repo.find_expired().await?;

    for row in expired {
        repo.set_status(row.id, AuthorityStatus::Expired).await?;

        tracing::info!(authority_id = %row.id, train_id = %row.train_id, "movement authority expired");

        let envelope = EventEnvelope::originate(
            "movement-authority-service",
            DomainEvent::MovementAuthorityExpired {
                authority_id: MovementAuthorityId::from(row.id),
                train_id: TrainId::from(row.train_id),
            },
        );
        let _ = state.mq.publish(&envelope).await;
    }

    Ok(())
}
