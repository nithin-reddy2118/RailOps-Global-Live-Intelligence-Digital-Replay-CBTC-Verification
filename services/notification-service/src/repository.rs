use crate::models::{AlertResponse, AlertSeverity};
use cbtc_errors::{AppError, AppResult};
use sqlx::PgPool;
use uuid::Uuid;

pub struct AlertRepository<'a> {
    pool: &'a PgPool,
}

const COLUMNS: &str = "id, severity, title, description, source_service, acknowledged, acknowledged_by, acknowledged_at, created_at, source_event_id";

impl<'a> AlertRepository<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    /// Creates an alert, deduplicating on `source_event_id` when the
    /// caller provides one (see the partial unique index in
    /// `20260101000009_notification_idempotency.sql`). Returns `Ok(None)`
    /// rather than an error when the insert was skipped as a duplicate —
    /// that's a normal, expected outcome of retry/duplicate delivery,
    /// not a failure.
    pub async fn create(
        &self,
        severity: AlertSeverity,
        title: &str,
        description: &str,
        source_service: &str,
        source_event_id: Option<Uuid>,
    ) -> AppResult<Option<AlertResponse>> {
        let query = format!(
            r#"
            INSERT INTO notification.alerts (severity, title, description, source_service, source_event_id)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (source_event_id) WHERE source_event_id IS NOT NULL DO NOTHING
            RETURNING {COLUMNS}
            "#
        );
        Ok(sqlx::query_as(&query)
            .bind(severity)
            .bind(title)
            .bind(description)
            .bind(source_service)
            .bind(source_event_id)
            .fetch_optional(self.pool)
            .await?)
    }

    pub async fn list_unacknowledged(&self) -> AppResult<Vec<AlertResponse>> {
        let query = format!(
            "SELECT {COLUMNS} FROM notification.alerts WHERE NOT acknowledged ORDER BY severity DESC, created_at DESC"
        );
        Ok(sqlx::query_as(&query).fetch_all(self.pool).await?)
    }

    pub async fn list_recent(&self, limit: i64) -> AppResult<Vec<AlertResponse>> {
        let query =
            format!("SELECT {COLUMNS} FROM notification.alerts ORDER BY created_at DESC LIMIT $1");
        Ok(sqlx::query_as(&query)
            .bind(limit)
            .fetch_all(self.pool)
            .await?)
    }

    pub async fn acknowledge(&self, id: Uuid, user_id: Uuid) -> AppResult<AlertResponse> {
        let query = format!(
            "UPDATE notification.alerts SET acknowledged = TRUE, acknowledged_by = $2, acknowledged_at = now() WHERE id = $1 RETURNING {COLUMNS}"
        );
        sqlx::query_as(&query)
            .bind(id)
            .bind(user_id)
            .fetch_optional(self.pool)
            .await?
            .ok_or_else(|| AppError::not_found("alert", id.to_string()))
    }
}
