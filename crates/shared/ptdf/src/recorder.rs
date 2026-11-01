//! PTDF (Production-Time Deterministic Forensics) recording.
//!
//! A "run" is one recording session — typically one demo scenario, one
//! CI race-condition reproduction, or one production incident window.
//! Every `EventEnvelope<DomainEvent>` that crosses the `EventEnvelope`
//! seam (see `cbtc-events`) during an active run is persisted here, in
//! the exact order Postgres assigns it (see the migration's comment on
//! why `BIGSERIAL` insertion order — not a logical clock — is the
//! recorded causal order).
//!
//! This crate does not itself hook into RabbitMQ; `cbtc-mq::MqClient`
//! holds an optional `Arc<PtdfRecorder>` + active `run_id` and calls
//! `record()` from its `publish()` method when recording is enabled
//! (see `crates/shared/mq/src/lib.rs`). That keeps recording an opt-in
//! concern per environment/run rather than a mandatory dependency every
//! service must always pay for.

use cbtc_events::{DomainEvent, EventEnvelope};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum PtdfError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("run {0} not found")]
    RunNotFound(Uuid),
    #[error("replay handler failed at sequence {sequence}: {source}")]
    ReplayHandlerFailed {
        sequence: i64,
        source: anyhow::Error,
    },
}

pub type PtdfResult<T> = Result<T, PtdfError>;

#[derive(Debug, Clone)]
pub struct PtdfRecorder {
    pool: PgPool,
}

impl PtdfRecorder {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Begin a new recording session. Returns the run's ID, which every
    /// subsequent `record()` call and eventual `replay()` call is keyed
    /// on.
    pub async fn start_run(&self, label: &str) -> PtdfResult<Uuid> {
        let (id,): (Uuid,) = sqlx::query_as(
            "INSERT INTO ptdf.runs (label, status) VALUES ($1, 'recording') RETURNING id",
        )
        .bind(label)
        .fetch_one(&self.pool)
        .await?;
        tracing::info!(run_id = %id, label, "PTDF recording started");
        Ok(id)
    }

    /// Persist one event as part of `run_id`. Called from
    /// `MqClient::publish` for every event published while a run is
    /// active — never call this directly from service business logic;
    /// go through the normal `EventEnvelope`/`MqClient::publish` path so
    /// every event (not just the ones a developer remembered to record)
    /// ends up in the trace.
    pub async fn record(
        &self,
        run_id: Uuid,
        envelope: &EventEnvelope<DomainEvent>,
    ) -> PtdfResult<()> {
        let payload: Value = serde_json::to_value(&envelope.payload)?;
        let event_type = envelope.payload.routing_key();

        sqlx::query(
            r#"
            INSERT INTO ptdf.trace_entries (run_id, event_id, causality_id, origin_service, event_type, payload)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(run_id)
        .bind(envelope.event_id)
        .bind(envelope.causality_id)
        .bind(&envelope.origin_service)
        .bind(event_type)
        .bind(payload)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Persist a generic platform/operational event in the active trace.
    pub async fn record_platform_event(
        &self,
        run_id: Uuid,
        event_type: &str,
        origin_service: &str,
        payload: Value,
    ) -> PtdfResult<()> {
        sqlx::query(
            r#"
            INSERT INTO ptdf.trace_entries
                (run_id, event_id, causality_id, origin_service, event_type, payload)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(run_id)
        .bind(Uuid::new_v4())
        .bind(Uuid::new_v4())
        .bind(origin_service)
        .bind(event_type)
        .bind(payload)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
    pub async fn end_run(&self, run_id: Uuid) -> PtdfResult<()> {
        let result = sqlx::query(
            "UPDATE ptdf.runs SET status = 'completed', ended_at = now() WHERE id = $1",
        )
        .bind(run_id)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(PtdfError::RunNotFound(run_id));
        }
        tracing::info!(run_id = %run_id, "PTDF recording ended");
        Ok(())
    }
}

