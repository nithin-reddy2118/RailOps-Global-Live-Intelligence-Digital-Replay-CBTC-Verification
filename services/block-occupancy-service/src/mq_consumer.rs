//! Consumes `cbtc.train.*` events and derives block occupancy from them.
//! This is the reactive half of block-occupancy-service: the HTTP API
//! (see `handlers.rs`) only serves reads and manual overrides — the
//! authoritative occupancy transitions driven by train movement happen
//! here, off the back of events published by train-service.

use crate::{
    repository::{BlockRepository, MoveOutcome},
    state::AppState,
};
use cbtc_domain::{BlockId, OccupancyState, TrainId};
use cbtc_errors::AppError;
use cbtc_events::{DomainEvent, EventEnvelope};

pub async fn run(state: AppState) -> Result<(), AppError> {
    let handler_state = state.clone();
    state
        .mq
        .consume(
            "block-occupancy-service.train-events",
            &["cbtc.train.position_reported", "cbtc.train.state_changed"],
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
    match envelope.payload {
        DomainEvent::TrainPositionReported {
            train_id, block_id, ..
        } => {
            let repo = BlockRepository::new(&state.pool);
            match repo
                .move_train_occupancy(train_id.as_uuid(), block_id.as_uuid())
                .await?
            {
                MoveOutcome::Applied(changes) => {
                    for (changed_block, prev, new) in changes {
                        let out_envelope = EventEnvelope::caused_by(
                            "block-occupancy-service",
                            &envelope,
                            DomainEvent::BlockOccupancyChanged {
                                block_id: BlockId::from(changed_block),
                                previous_state: format!("{prev:?}"),
                                new_state: format!("{new:?}"),
                                train_id: Some(train_id),
                            },
                        );
                        if let Err(e) = state.mq.publish(&out_envelope).await {
                            tracing::warn!(error = %e, "failed to publish derived BlockOccupancyChanged");
                        }
                    }
                }
                MoveOutcome::Conflict { incumbent_train_id } => {
                    // Never silently overwrite: the block's occupancy
                    // was left exactly as it was. Surface the anomaly
                    // loudly instead — notification-service alerts on
                    // this event type, and the reporting train is
                    // braked as a precaution since its own understanding
                    // of where it is has just been shown to be
                    // inconsistent with recorded network state.
                    tracing::error!(
                        block_id = %block_id,
                        incumbent_train_id = %incumbent_train_id,
                        reporting_train_id = %train_id,
                        "block occupancy conflict: refusing to overwrite incumbent occupant"
                    );

                    let conflict_envelope = EventEnvelope::caused_by(
                        "block-occupancy-service",
                        &envelope,
                        DomainEvent::BlockOccupancyConflict {
                            block_id,
                            incumbent_train_id: TrainId::from(incumbent_train_id),
                            reporting_train_id: train_id,
                        },
                    );
                    let _ = state.mq.publish(&conflict_envelope).await;

                    let brake_envelope = EventEnvelope::caused_by(
                        "block-occupancy-service",
                        &conflict_envelope,
                        DomainEvent::EmergencyBrakeTriggered {
                            train_id,
                            reason: format!("reported occupancy conflict with block {block_id}"),
                        },
                    );
                    let _ = state.mq.publish(&brake_envelope).await;
                }
            }
        }
        DomainEvent::TrainStateChanged {
            train_id,
            ref new_state,
            ..
        } if new_state == "CommunicationLost" => {
            // A train that has stopped communicating can no longer be
            // trusted to actually be where its last report said — mark
            // whatever block it's currently recorded against as
            // `Ambiguous` rather than leaving it falsely `Occupied`
            // (which would at least still block new authorities through
            // it) or `Clear` (which would be actively unsafe).
            let repo = BlockRepository::new(&state.pool);
            let occupied_blocks = repo.find_by_train(train_id.as_uuid()).await?;
            for block in occupied_blocks {
                if block.occupancy != OccupancyState::Ambiguous {
                    repo.set_manual_occupancy(block.id, OccupancyState::Ambiguous)
                        .await?;
                    let out_envelope = EventEnvelope::caused_by(
                        "block-occupancy-service",
                        &envelope,
                        DomainEvent::BlockOccupancyChanged {
                            block_id: BlockId::from(block.id),
                            previous_state: format!("{:?}", block.occupancy),
                            new_state: "Ambiguous".to_string(),
                            train_id: Some(train_id),
                        },
                    );
                    let _ = state.mq.publish(&out_envelope).await;
                }
            }
        }
        _ => {}
    }
    Ok(())
}
