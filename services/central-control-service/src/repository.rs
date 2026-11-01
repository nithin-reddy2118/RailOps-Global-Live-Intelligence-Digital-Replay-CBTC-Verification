use crate::models::{
    AuthorityOverview, BlockOverview, SignalOverview, SwitchOverview, TrainOverview,
    UnacknowledgedAlertOverview,
};
use cbtc_errors::AppResult;
use sqlx::PgPool;
use uuid::Uuid;

pub struct OverviewRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> OverviewRepository<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn trains(&self) -> AppResult<Vec<TrainOverview>> {
        Ok(sqlx::query_as(
            "SELECT id, identifier, operational_state::text AS operational_state, current_block, last_heartbeat FROM train.trains ORDER BY identifier",
        )
        .fetch_all(self.pool)
        .await?)
    }

    pub async fn blocks(&self) -> AppResult<Vec<BlockOverview>> {
        Ok(sqlx::query_as(
            "SELECT id, name, occupancy::text AS occupancy, occupied_by FROM signaling.blocks ORDER BY name",
        )
        .fetch_all(self.pool)
        .await?)
    }

    pub async fn signals(&self) -> AppResult<Vec<SignalOverview>> {
        Ok(sqlx::query_as(
            "SELECT id, name, current_aspect::text AS current_aspect, protects_block FROM signaling.signals ORDER BY name",
        )
        .fetch_all(self.pool)
        .await?)
    }

    pub async fn switches(&self) -> AppResult<Vec<SwitchOverview>> {
        Ok(sqlx::query_as(
            "SELECT id, name, position::text AS position, locked FROM signaling.switches ORDER BY name",
        )
        .fetch_all(self.pool)
        .await?)
    }

    pub async fn active_authorities(&self) -> AppResult<Vec<AuthorityOverview>> {
        Ok(sqlx::query_as(
            "SELECT id, train_id, status::text AS status, expires_at FROM train.movement_authorities WHERE status IN ('granted', 'active') ORDER BY expires_at",
        )
        .fetch_all(self.pool)
        .await?)
    }

    pub async fn unacknowledged_alerts(&self) -> AppResult<Vec<UnacknowledgedAlertOverview>> {
        Ok(sqlx::query_as(
            "SELECT id, severity::text AS severity, title, created_at FROM notification.alerts WHERE NOT acknowledged ORDER BY created_at DESC LIMIT 50",
        )
        .fetch_all(self.pool)
        .await?)
    }

    pub async fn active_authority_ids(&self) -> AppResult<Vec<Uuid>> {
        let rows: Vec<(Uuid,)> = sqlx::query_as(
            "SELECT id FROM train.movement_authorities WHERE status IN ('granted', 'active')",
        )
        .fetch_all(self.pool)
        .await?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }
}
