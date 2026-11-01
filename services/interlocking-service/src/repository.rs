use cbtc_domain::{BlockId, OccupancyState, SwitchPosition};
use cbtc_errors::AppResult;
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

pub struct InterlockingRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> InterlockingRepository<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn get_train_current_block(&self, train_id: Uuid) -> AppResult<Option<Uuid>> {
        let row: Option<(Option<Uuid>,)> =
            sqlx::query_as("SELECT current_block FROM train.trains WHERE id = $1")
                .bind(train_id)
                .fetch_optional(self.pool)
                .await?;
        Ok(row.and_then(|(b,)| b))
    }

    pub async fn occupancy_for_blocks(
        &self,
        blocks: &[Uuid],
    ) -> AppResult<HashMap<BlockId, OccupancyState>> {
        if blocks.is_empty() {
            return Ok(HashMap::new());
        }
        let rows: Vec<(Uuid, OccupancyState)> =
            sqlx::query_as("SELECT id, occupancy FROM signaling.blocks WHERE id = ANY($1)")
                .bind(blocks)
                .fetch_all(self.pool)
                .await?;
        Ok(rows
            .into_iter()
            .map(|(id, occ)| (BlockId::from(id), occ))
            .collect())
    }

    /// Every block currently covered by any other train's live
    /// (`granted`/`active`) movement authority, excluding `exclude_train`.
    pub async fn blocks_under_other_active_authorities(
        &self,
        exclude_train: Uuid,
    ) -> AppResult<HashSet<BlockId>> {
        let rows: Vec<(Vec<Uuid>,)> = sqlx::query_as(
            "SELECT block_sequence FROM train.movement_authorities WHERE status IN ('granted', 'active') AND train_id != $1",
        )
        .bind(exclude_train)
        .fetch_all(self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .flat_map(|(seq,)| seq.into_iter().map(BlockId::from))
            .collect())
    }

    /// For a candidate path, returns the (block, switch_position) pairs
    /// for every switch that gates entry to a block in the path.
    pub async fn switch_positions_gating_path(
        &self,
        path: &[Uuid],
    ) -> AppResult<Vec<(BlockId, SwitchPosition)>> {
        if path.len() < 2 {
            return Ok(Vec::new());
        }
        let rows: Vec<(Uuid, Uuid, SwitchPosition)> = sqlx::query_as(
            r#"
            SELECT a.to_block, sw.id, sw.position
            FROM signaling.block_adjacency a
            JOIN signaling.switches sw ON sw.id = a.via_switch
            WHERE a.via_switch IS NOT NULL AND a.from_block = ANY($1) AND a.to_block = ANY($1)
            "#,
        )
        .bind(path)
        .fetch_all(self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(to_block, _switch_id, pos)| (BlockId::from(to_block), pos))
            .collect())
    }

    pub async fn get_switch_position(
        &self,
        switch_id: Uuid,
    ) -> AppResult<Option<(SwitchPosition, bool)>> {
        let row: Option<(SwitchPosition, bool)> =
            sqlx::query_as("SELECT position, locked FROM signaling.switches WHERE id = $1")
                .bind(switch_id)
                .fetch_optional(self.pool)
                .await?;
        Ok(row)
    }

    pub async fn set_switch_position(
        &self,
        switch_id: Uuid,
        position: SwitchPosition,
    ) -> AppResult<()> {
        sqlx::query(
            "UPDATE signaling.switches SET position = $2, last_moved_at = now() WHERE id = $1",
        )
        .bind(switch_id)
        .bind(position)
        .execute(self.pool)
        .await?;
        Ok(())
    }
}
