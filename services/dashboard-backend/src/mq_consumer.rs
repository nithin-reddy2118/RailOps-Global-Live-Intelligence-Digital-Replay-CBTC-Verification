use crate::state::AppState;
use cbtc_errors::AppError;
use cbtc_events::{DomainEvent, EventEnvelope};
use cbtc_mq::MqClient;

pub async fn run(mq: std::sync::Arc<MqClient>, state: AppState) -> Result<(), AppError> {
    mq.consume(
        "dashboard-backend.all-events",
        &["cbtc.#"],
        move |envelope| {
            let state = state.clone();
            async move { forward(&state, envelope).await }
        },
    )
    .await
}

async fn forward(state: &AppState, envelope: EventEnvelope<DomainEvent>) -> Result<(), AppError> {
    let active_run = *state.active_run_id.read().await;
    if let Some(run_id) = active_run {
        if let Err(e) = state.ptdf.record(run_id, &envelope).await {
            tracing::warn!(error = %e, run_id = %run_id, event_id = %envelope.event_id, "PTDF recording failed for dashboard event");
        }
    }

    match serde_json::to_string(&envelope) {
        Ok(json) => {
            let _ = state.event_tx.send(json);
        }
        Err(e) => tracing::warn!(error = %e, "failed to serialize event for dashboard broadcast"),
    }
    Ok(())
}
