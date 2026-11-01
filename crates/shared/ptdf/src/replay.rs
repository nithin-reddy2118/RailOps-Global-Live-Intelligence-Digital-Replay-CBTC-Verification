//! Deterministic replay: loads every trace entry for a run, in the
//! exact order it was recorded, and hands each one to a caller-supplied
//! async handler *sequentially* — the next entry is never dispatched
//! until the handler's future for the current one resolves. That
//! sequential-await is what makes replay deterministic: the same input
//! sequence, applied one at a time to freshly-initialized service
//! state, reproduces the same sequence of state transitions, because
//! every service in this platform (see `interlocking-service/src/safety.rs`,
//! `block-occupancy-service/src/repository.rs`) computes its next state
//! as a pure function of (current DB state, one incoming event) — the
//! only non-determinism the platform introduces is wall-clock timing
//! and `cbtc-config::FaultInjectionConfig`'s randomness, both of which
//! are absent from a replay run (there's no real MQ round-trip, so no
//! injected latency/loss/duplication applies).

use cbtc_events::{DomainEvent, EventEnvelope};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::recorder::{PtdfError, PtdfResult};

#[derive(Debug, Clone)]
pub struct RecordedEntry {
    pub sequence: i64,
    pub event_id: Uuid,
    pub causality_id: Uuid,
    pub origin_service: String,
    pub event_type: String,
    pub recorded_at: DateTime<Utc>,
    pub envelope: EventEnvelope<DomainEvent>,
}

#[derive(Debug, Clone)]
pub struct PtdfReplayer {
    pool: PgPool,
}

impl PtdfReplayer {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Load every trace entry for `run_id`, ordered by recorded sequence
    /// (see the migration's note on why insertion order is the causal
    /// order). Fails fast on any entry whose payload no longer
    /// deserializes into the current `DomainEvent` enum — a replay of a
    /// trace recorded against an older schema version is a real failure
    /// mode worth surfacing loudly, not silently skipping entries.
    pub async fn load_run(&self, run_id: Uuid) -> PtdfResult<Vec<RecordedEntry>> {
        let run_exists: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM ptdf.runs WHERE id = $1")
            .bind(run_id)
            .fetch_optional(&self.pool)
            .await?;
        if run_exists.is_none() {
            return Err(PtdfError::RunNotFound(run_id));
        }

        #[allow(clippy::type_complexity)]
        let rows: Vec<(
            i64,
            Uuid,
            Uuid,
            String,
            String,
            serde_json::Value,
            DateTime<Utc>,
        )> = sqlx::query_as(
            r#"
            SELECT id, event_id, causality_id, origin_service, event_type, payload, recorded_at
            FROM ptdf.trace_entries
            WHERE run_id = $1
            ORDER BY id ASC
            "#,
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await?;

        let mut entries = Vec::with_capacity(rows.len());
        for (sequence, event_id, causality_id, origin_service, event_type, payload, recorded_at) in
            rows
        {
            let payload: DomainEvent = serde_json::from_value(payload)?;
            entries.push(RecordedEntry {
                sequence,
                event_id,
                causality_id,
                origin_service: origin_service.clone(),
                event_type,
                recorded_at,
                envelope: EventEnvelope {
                    event_id,
                    causality_id,
                    // `origin_service` on EventEnvelope is `&'static str`
                    // (see cbtc-events) because live services only ever
                    // construct it from a compile-time literal; replay
                    // reconstructs from a stored owned `String` instead,
                    // so it leaks a small, bounded, one-time string per
                    // replayed entry to obtain a `&'static str` — replay
                    // runs are diagnostic tooling invocations, not a
                    // long-running service loop, so this is an
                    // acceptable, deliberate tradeoff rather than an
                    // oversight.
                    origin_service,
                    occurred_at: recorded_at,
                    payload,
                },
            });
        }

        Ok(entries)
    }

    /// Replay `run_id`'s trace sequentially through `handler`. Stops and
    /// returns the first handler error rather than continuing past it —
    /// a replay that silently swallows a mid-sequence failure would
    /// defeat the entire point of using replay to pin down where a bug
    /// reproduces.
    pub async fn replay<F, Fut>(&self, run_id: Uuid, mut handler: F) -> PtdfResult<usize>
    where
        F: FnMut(RecordedEntry) -> Fut,
        Fut: std::future::Future<Output = Result<(), anyhow::Error>>,
    {
        sqlx::query("UPDATE ptdf.runs SET status = 'replaying' WHERE id = $1")
            .bind(run_id)
            .execute(&self.pool)
            .await?;

        let entries = self.load_run(run_id).await?;
        let total = entries.len();

        for entry in entries {
            let sequence = entry.sequence;
            if let Err(e) = handler(entry).await {
                tracing::error!(run_id = %run_id, sequence, error = %e, "replay handler failed");
                return Err(PtdfError::ReplayHandlerFailed {
                    sequence,
                    source: e,
                });
            }
        }

        sqlx::query("UPDATE ptdf.runs SET status = 'replayed' WHERE id = $1")
            .bind(run_id)
            .execute(&self.pool)
            .await?;

        tracing::info!(run_id = %run_id, entries_replayed = total, "PTDF replay completed");
        Ok(total)
    }
}
