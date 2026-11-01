use crate::models::{TicketResponse, TicketStatus};
use cbtc_errors::{AppError, AppResult};
use sqlx::PgPool;
use uuid::Uuid;

const COLUMNS: &str = "id, asset_description, related_block, related_switch, related_signal, status, opened_by, opened_at, resolved_at, notes";

pub struct TicketRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> TicketRepository<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn open(
        &self,
        asset_description: &str,
        related_block: Option<Uuid>,
        related_switch: Option<Uuid>,
        related_signal: Option<Uuid>,
        opened_by: Uuid,
    ) -> AppResult<TicketResponse> {
        let query = format!(
            r#"
            INSERT INTO maintenance.tickets (asset_description, related_block, related_switch, related_signal, opened_by)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING {COLUMNS}
            "#
        );
        Ok(sqlx::query_as(&query)
            .bind(asset_description)
            .bind(related_block)
            .bind(related_switch)
            .bind(related_signal)
            .bind(opened_by)
            .fetch_one(self.pool)
            .await?)
    }

    pub async fn list_open(&self) -> AppResult<Vec<TicketResponse>> {
        let query = format!(
            "SELECT {COLUMNS} FROM maintenance.tickets WHERE status IN ('open', 'in_progress') ORDER BY opened_at ASC"
        );
        Ok(sqlx::query_as(&query).fetch_all(self.pool).await?)
    }

    pub async fn list_all(&self, limit: i64) -> AppResult<Vec<TicketResponse>> {
        let query =
            format!("SELECT {COLUMNS} FROM maintenance.tickets ORDER BY opened_at DESC LIMIT $1");
        Ok(sqlx::query_as(&query)
            .bind(limit)
            .fetch_all(self.pool)
            .await?)
    }

    pub async fn find_by_id(&self, id: Uuid) -> AppResult<Option<TicketResponse>> {
        let query = format!("SELECT {COLUMNS} FROM maintenance.tickets WHERE id = $1");
        Ok(sqlx::query_as(&query)
            .bind(id)
            .fetch_optional(self.pool)
            .await?)
    }

    pub async fn update_status(
        &self,
        id: Uuid,
        status: TicketStatus,
        notes: Option<&str>,
    ) -> AppResult<TicketResponse> {
        let resolved_at_clause = if matches!(status, TicketStatus::Resolved | TicketStatus::Closed)
        {
            "resolved_at = now()"
        } else {
            "resolved_at = NULL"
        };
        let query = format!(
            "UPDATE maintenance.tickets SET status = $2, notes = COALESCE($3, notes), {resolved_at_clause} WHERE id = $1 RETURNING {COLUMNS}"
        );
        sqlx::query_as(&query)
            .bind(id)
            .bind(status)
            .bind(notes)
            .fetch_optional(self.pool)
            .await?
            .ok_or_else(|| AppError::not_found("maintenance_ticket", id.to_string()))
    }
}
