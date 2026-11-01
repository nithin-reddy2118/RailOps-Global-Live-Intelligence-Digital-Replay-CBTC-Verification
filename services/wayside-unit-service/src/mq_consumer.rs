use crate::{repository::WaysideRepository, signal_logic, state::AppState};
use cbtc_domain::SignalId;
use cbtc_errors::AppError;
use cbtc_events::{DomainEvent, EventEnvelope};

pub async fn run(state: AppState) -> Result<(), AppError> {
    let handler_state = state.clone();
    state
        .mq
        .consume(
            "wayside-unit-service.occupancy-events",
            &["cbtc.block.occupancy_changed"],
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
    let DomainEvent::BlockOccupancyChanged { block_id, .. } = &envelope.payload else {
        return Ok(());
    };

    let repo = WaysideRepository::new(&state.pool);
    let signals = repo.signals_protecting_block(block_id.as_uuid()).await?;

    for (signal_id, previous_aspect) in signals {
        let protected_occupancy = repo.occupancy_of(block_id.as_uuid()).await?;
        let next_occupancy = repo
            .most_restrictive_successor_occupancy(block_id.as_uuid())
            .await?;
        let new_aspect = signal_logic::compute_aspect(protected_occupancy, next_occupancy);

        if new_aspect == previous_aspect {
            continue;
        }

        repo.set_aspect(signal_id, new_aspect).await?;

        let out_envelope = EventEnvelope::caused_by(
            "wayside-unit-service",
            &envelope,
            DomainEvent::SignalAspectChanged {
                signal_id: SignalId::from(signal_id),
                previous_aspect: format!("{previous_aspect:?}"),
                new_aspect: format!("{new_aspect:?}"),
            },
        );
        if let Err(e) = state.mq.publish(&out_envelope).await {
            tracing::warn!(error = %e, signal_id = %signal_id, "failed to publish SignalAspectChanged");
        }

        tracing::info!(signal_id = %signal_id, previous = ?previous_aspect, new = ?new_aspect, "signal aspect recomputed");
    }

    Ok(())
}
