use cbtc_domain::TrainOperationalState;
use cbtc_errors::{AppError, AppResult};
use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, FromRow)]
pub struct TrainRow {
    pub id: Uuid,
    pub identifier: String,
    pub operational_state: TrainOperationalState,
    pub current_block: Option<Uuid>,
    pub active_authority: Option<Uuid>,
    pub max_speed_kph: i32,
    pub length_m: f64,
    pub last_heartbeat: DateTime<Utc>,
}

pub struct TrainRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> TrainRepository<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        identifier: &str,
        max_speed_kph: i32,
        length_m: f64,
        initial_block: Option<Uuid>,
    ) -> AppResult<TrainRow> {
        sqlx::query_as::<_, TrainRow>(
            r#"
            INSERT INTO train.trains (identifier, operational_state, current_block, max_speed_kph, length_m)
            VALUES ($1, 'staged', $2, $3, $4)
            RETURNING id, identifier, operational_state, current_block, active_authority, max_speed_kph, length_m, last_heartbeat
            "#,
        )
        .bind(identifier)
        .bind(initial_block)
        .bind(max_speed_kph)
        .bind(length_m)
        .fetch_one(self.pool)
        .await
        .map_err(|e| match &e {
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                AppError::conflict("a train with this identifier already exists")
            }
            _ => AppError::from(e),
        })
    }

    pub async fn find_by_id(&self, id: Uuid) -> AppResult<Option<TrainRow>> {
        Ok(sqlx::query_as::<_, TrainRow>(
            r#"SELECT id, identifier, operational_state, current_block, active_authority, max_speed_kph, length_m, last_heartbeat
               FROM train.trains WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(self.pool)
        .await?)
    }

    pub async fn list_all(&self) -> AppResult<Vec<TrainRow>> {
        Ok(sqlx::query_as::<_, TrainRow>(
            r#"SELECT id, identifier, operational_state, current_block, active_authority, max_speed_kph, length_m, last_heartbeat
               FROM train.trains ORDER BY identifier"#,
        )
        .fetch_all(self.pool)
        .await?)
    }

    /// List trains whose last heartbeat is older than `timeout_secs` and
    /// whose state isn't already `communication_lost` — used by the
    /// background heartbeat monitor to find newly-stale trains exactly
    /// once per staleness transition rather than repeatedly.
    pub async fn list_newly_stale(&self, timeout_secs: i64) -> AppResult<Vec<TrainRow>> {
        Ok(sqlx::query_as::<_, TrainRow>(
            r#"
            SELECT id, identifier, operational_state, current_block, active_authority, max_speed_kph, length_m, last_heartbeat
            FROM train.trains
            WHERE last_heartbeat < now() - ($1 || ' seconds')::interval
              AND operational_state != 'communication_lost'
              AND operational_state != 'out_of_service'
            "#,
        )
        .bind(timeout_secs.to_string())
        .fetch_all(self.pool)
        .await?)
    }

    /// Records a position report, but only if it's not stale/out-of-order
    /// relative to the most recent report already recorded for this
    /// train (Phase 8). Comparison uses `client_timestamp` — the
    /// train's own clock, not server arrival order — since network
    /// reordering (a real, injected failure mode; see
    /// `crates/shared/mq/src/lib.rs`'s `message_delay_ms_max`) means a
    /// report can arrive at the server after a logically later one.
    ///
    /// The whole check-then-write is one transaction with the train row
    /// locked `FOR UPDATE` first, so two concurrent reports for the same
    /// train (e.g. one delayed by fault injection, arriving interleaved
    /// with a normal one) can't both read "no newer report exists yet"
    /// and both proceed to write — exactly the same class of race
    /// `movement-authority-service::grant_if_no_conflict` closes for
    /// authorities.
    ///
    /// Returns `Ok(true)` if the report was applied, `Ok(false)` if it
    /// was rejected as stale/out-of-order (the caller surfaces this as a
    /// structured conflict rather than pretending the write happened).
    pub async fn record_position_report(
        &self,
        train_id: Uuid,
        occupied_block: Uuid,
        offset_within_block_m: f64,
        speed_kph: f64,
        acceleration_mps2: f64,
        client_timestamp: DateTime<Utc>,
    ) -> AppResult<bool> {
        let mut tx = self.pool.begin().await?;

        sqlx::query("SELECT id FROM train.trains WHERE id = $1 FOR UPDATE")
            .bind(train_id)
            .fetch_optional(&mut *tx)
            .await?;

        let latest: Option<(DateTime<Utc>,)> = sqlx::query_as(
            "SELECT client_timestamp FROM train.position_reports WHERE train_id = $1 ORDER BY client_timestamp DESC LIMIT 1",
        )
        .bind(train_id)
        .fetch_optional(&mut *tx)
        .await?;

        if let Some((latest_client_timestamp,)) = latest {
            if client_timestamp <= latest_client_timestamp {
                // Still counts as proof of life even though the position
                // itself is discarded — a reordered-but-arriving stream
                // must not trip the heartbeat-loss monitor just because
                // this particular message lost the ordering race.
                sqlx::query("UPDATE train.trains SET last_heartbeat = now() WHERE id = $1")
                    .bind(train_id)
                    .execute(&mut *tx)
                    .await?;
                tx.commit().await?;
                return Ok(false);
            }
        }

        sqlx::query(
            r#"
            INSERT INTO train.position_reports (train_id, occupied_block, offset_within_block_m, speed_kph, acceleration_mps2, client_timestamp)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(train_id)
        .bind(occupied_block)
        .bind(offset_within_block_m)
        .bind(speed_kph)
        .bind(acceleration_mps2)
        .bind(client_timestamp)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"UPDATE train.trains SET current_block = $2, last_heartbeat = now() WHERE id = $1"#,
        )
        .bind(train_id)
        .bind(occupied_block)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(true)
    }

    pub async fn touch_heartbeat(&self, train_id: Uuid) -> AppResult<()> {
        sqlx::query(r#"UPDATE train.trains SET last_heartbeat = now() WHERE id = $1"#)
            .bind(train_id)
            .execute(self.pool)
            .await?;
        Ok(())
    }

    pub async fn set_state(
        &self,
        train_id: Uuid,
        new_state: TrainOperationalState,
    ) -> AppResult<()> {
        sqlx::query(r#"UPDATE train.trains SET operational_state = $2 WHERE id = $1"#)
            .bind(train_id)
            .bind(new_state)
            .execute(self.pool)
            .await?;
        Ok(())
    }
}
