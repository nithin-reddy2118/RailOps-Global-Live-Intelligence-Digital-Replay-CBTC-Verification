use crate::models::AuditEntryResponse;
use cbtc_errors::AppResult;
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

pub struct AuditRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> AuditRepository<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn insert(
        &self,
        event_id: Uuid,
        causality_id: Uuid,
        origin_service: &str,
        action: &str,
        details: Value,
    ) -> AppResult<()> {
        sqlx::query(
            r#"
            INSERT INTO audit.log_entries (event_id, causality_id, origin_service, action, details)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(event_id)
        .bind(causality_id)
        .bind(origin_service)
        .bind(action)
        .bind(details)
        .execute(self.pool)
        .await?;
        Ok(())
    }

    pub async fn recent(&self, limit: i64) -> AppResult<Vec<AuditEntryResponse>> {
        let rows = sqlx::query_as::<_, (i64, Uuid, Uuid, String, Option<Uuid>, String, Value, chrono::DateTime<chrono::Utc>)>(
            r#"
            SELECT id, event_id, causality_id, origin_service, actor_user_id, action, details, occurred_at
            FROM audit.log_entries
            ORDER BY occurred_at DESC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(
                |(
                    id,
                    event_id,
                    causality_id,
                    origin_service,
                    actor_user_id,
                    action,
                    details,
                    occurred_at,
                )| {
                    AuditEntryResponse {
                        id,
                        event_id,
                        causality_id,
                        origin_service,
                        actor_user_id,
                        action,
                        details,
                        occurred_at,
                    }
                },
            )
            .collect())
    }

    pub async fn by_causality(&self, causality_id: Uuid) -> AppResult<Vec<AuditEntryResponse>> {
        let rows = sqlx::query_as::<_, (i64, Uuid, Uuid, String, Option<Uuid>, String, Value, chrono::DateTime<chrono::Utc>)>(
            r#"
            SELECT id, event_id, causality_id, origin_service, actor_user_id, action, details, occurred_at
            FROM audit.log_entries
            WHERE causality_id = $1
            ORDER BY occurred_at ASC
            "#,
        )
        .bind(causality_id)
        .fetch_all(self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(
                |(
                    id,
                    event_id,
                    causality_id,
                    origin_service,
                    actor_user_id,
                    action,
                    details,
                    occurred_at,
                )| {
                    AuditEntryResponse {
                        id,
                        event_id,
                        causality_id,
                        origin_service,
                        actor_user_id,
                        action,
                        details,
                        occurred_at,
                    }
                },
            )
            .collect())
    }
}
