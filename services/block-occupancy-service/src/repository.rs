use cbtc_domain::OccupancyState;
use cbtc_errors::AppResult;
use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, FromRow, Clone)]
pub struct BlockRow {
    pub id: Uuid,
    pub name: String,
    pub length_m: f64,
    pub occupancy: OccupancyState,
    pub occupied_by: Option<Uuid>,
    pub last_updated: DateTime<Utc>,
    pub max_speed_kph: i32,
    pub bidirectional: bool,
}

pub struct BlockRepository<'a> {
    pool: &'a PgPool,
}

const BLOCK_COLUMNS: &str =
    "id, name, length_m, occupancy, occupied_by, last_updated, max_speed_kph, bidirectional";

/// Outcome of attempting to move a train's occupancy into a new block.
#[derive(Debug)]
pub enum MoveOutcome {
    Applied(Vec<(Uuid, OccupancyState, OccupancyState)>),
    /// The target block is already occupied by a DIFFERENT train, per
    /// its last known state. Nothing was written — see the safety note
    /// on `move_train_occupancy`.
    Conflict {
        incumbent_train_id: Uuid,
    },
}

impl<'a> BlockRepository<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn list_all(&self) -> AppResult<Vec<BlockRow>> {
        let query = format!("SELECT {BLOCK_COLUMNS} FROM signaling.blocks ORDER BY name");
        Ok(sqlx::query_as::<_, BlockRow>(&query)
            .fetch_all(self.pool)
            .await?)
    }

    pub async fn find_by_id(&self, id: Uuid) -> AppResult<Option<BlockRow>> {
        let query = format!("SELECT {BLOCK_COLUMNS} FROM signaling.blocks WHERE id = $1");
        Ok(sqlx::query_as::<_, BlockRow>(&query)
            .bind(id)
            .fetch_optional(self.pool)
            .await?)
    }

    pub async fn find_by_train(&self, train_id: Uuid) -> AppResult<Vec<BlockRow>> {
        let query = format!("SELECT {BLOCK_COLUMNS} FROM signaling.blocks WHERE occupied_by = $1");
        Ok(sqlx::query_as::<_, BlockRow>(&query)
            .bind(train_id)
            .fetch_all(self.pool)
            .await?)
    }

    pub async fn set_manual_occupancy(&self, id: Uuid, occupancy: OccupancyState) -> AppResult<()> {
        sqlx::query(
            "UPDATE signaling.blocks SET occupancy = $2, occupied_by = NULL, last_updated = now() WHERE id = $1",
        )
        .bind(id)
        .bind(occupancy)
        .execute(self.pool)
        .await?;
        Ok(())
    }

    /// Atomically: clear every block currently occupied by `train_id`
    /// other than `new_block`, then mark `new_block` occupied by
    /// `train_id` — UNLESS `new_block` is already occupied by a
    /// DIFFERENT train, in which case this is a genuine occupancy
    /// conflict (duplicate/ghost report, sensor fault, or an actual
    /// dual occupancy) and nothing is written: `occupied_by` is never
    /// silently overwritten out from under an incumbent train. The
    /// caller (see `mq_consumer.rs`) publishes `BlockOccupancyConflict`
    /// instead of the normal `BlockOccupancyChanged` in that case.
    ///
    /// The target block is locked `FOR UPDATE` before the conflict check
    /// so two trains' reports racing for the same block can't both read
    /// "currently clear" and both proceed to claim it.
    pub async fn move_train_occupancy(
        &self,
        train_id: Uuid,
        new_block: Uuid,
    ) -> AppResult<MoveOutcome> {
        let mut tx = self.pool.begin().await?;
        let mut changes = Vec::new();

        let current: (OccupancyState, Option<Uuid>) = sqlx::query_as(
            "SELECT occupancy, occupied_by FROM signaling.blocks WHERE id = $1 FOR UPDATE",
        )
        .bind(new_block)
        .fetch_one(&mut *tx)
        .await?;

        if current.0 == OccupancyState::Occupied {
            if let Some(incumbent) = current.1 {
                if incumbent != train_id {
                    tx.rollback().await?;
                    return Ok(MoveOutcome::Conflict {
                        incumbent_train_id: incumbent,
                    });
                }
            }
        }

        let previously_occupied: Vec<(Uuid, OccupancyState)> = sqlx::query_as(
            "SELECT id, occupancy FROM signaling.blocks WHERE occupied_by = $1 AND id != $2",
        )
        .bind(train_id)
        .bind(new_block)
        .fetch_all(&mut *tx)
        .await?;

        for (block_id, prev_state) in previously_occupied {
            sqlx::query(
                "UPDATE signaling.blocks SET occupancy = 'clear', occupied_by = NULL, last_updated = now() WHERE id = $1",
            )
            .bind(block_id)
            .execute(&mut *tx)
            .await?;
            changes.push((block_id, prev_state, OccupancyState::Clear));
        }

        if current.0 != OccupancyState::Occupied || current.1 != Some(train_id) {
            sqlx::query(
                "UPDATE signaling.blocks SET occupancy = 'occupied', occupied_by = $2, last_updated = now() WHERE id = $1",
            )
            .bind(new_block)
            .bind(train_id)
            .execute(&mut *tx)
            .await?;
            changes.push((new_block, current.0, OccupancyState::Occupied));
        }

        tx.commit().await?;
        Ok(MoveOutcome::Applied(changes))
    }
}
