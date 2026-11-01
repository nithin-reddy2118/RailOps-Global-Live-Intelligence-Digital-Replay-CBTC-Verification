use cbtc_domain::{OccupancyState, SignalAspect};
use cbtc_errors::AppResult;
use sqlx::PgPool;
use uuid::Uuid;

pub struct WaysideRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> WaysideRepository<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    /// Every signal that protects `block` — i.e. whose aspect must be
    /// recomputed when `block`'s occupancy changes.
    pub async fn signals_protecting_block(
        &self,
        block: Uuid,
    ) -> AppResult<Vec<(Uuid, SignalAspect)>> {
        Ok(sqlx::query_as(
            "SELECT id, current_aspect FROM signaling.signals WHERE protects_block = $1",
        )
        .bind(block)
        .fetch_all(self.pool)
        .await?)
    }

    pub async fn occupancy_of(&self, block: Uuid) -> AppResult<Option<OccupancyState>> {
        let row: Option<(OccupancyState,)> =
            sqlx::query_as("SELECT occupancy FROM signaling.blocks WHERE id = $1")
                .bind(block)
                .fetch_optional(self.pool)
                .await?;
        Ok(row.map(|(o,)| o))
    }

    /// The block(s) immediately downstream of `block` via unconditional
    /// or any-position adjacency — used for the second lookahead step.
    /// Only the first successor is used by `signal_logic::compute_aspect`
    /// (single-yellow variant); multiple successors (a diverging switch)
    /// are each checked and the most restrictive occupancy wins.
    pub async fn most_restrictive_successor_occupancy(
        &self,
        block: Uuid,
    ) -> AppResult<Option<OccupancyState>> {
        let rows: Vec<(OccupancyState,)> = sqlx::query_as(
            r#"
            SELECT b.occupancy
            FROM signaling.block_adjacency a
            JOIN signaling.blocks b ON b.id = a.to_block
            WHERE a.from_block = $1
            "#,
        )
        .bind(block)
        .fetch_all(self.pool)
        .await?;

        Ok(rows.into_iter().map(|(o,)| o).max_by_key(|o| match o {
            OccupancyState::Clear => 0,
            OccupancyState::Ambiguous => 1,
            OccupancyState::Occupied => 2,
            OccupancyState::OutOfService => 3,
        }))
    }

    pub async fn set_aspect(&self, signal_id: Uuid, aspect: SignalAspect) -> AppResult<()> {
        sqlx::query("UPDATE signaling.signals SET current_aspect = $2, last_aspect_change = now() WHERE id = $1")
            .bind(signal_id)
            .bind(aspect)
            .execute(self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_signal(&self, id: Uuid) -> AppResult<Option<(Uuid, SignalAspect)>> {
        let row: Option<(Uuid, SignalAspect)> = sqlx::query_as(
            "SELECT protects_block, current_aspect FROM signaling.signals WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(self.pool)
        .await?;
        Ok(row)
    }
}
