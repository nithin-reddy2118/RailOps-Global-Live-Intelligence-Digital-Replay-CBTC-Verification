use cbtc_domain::AuthorityStatus;
use cbtc_errors::AppResult;
use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, FromRow, Clone)]
pub struct AuthorityRow {
    pub id: Uuid,
    pub train_id: Uuid,
    pub route_id: Option<Uuid>,
    pub block_sequence: Vec<Uuid>,
    pub limit_of_movement_m: f64,
    pub max_authorized_speed_kph: i32,
    pub status: AuthorityStatus,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

const AUTHORITY_COLUMNS: &str = "id, train_id, route_id, block_sequence, limit_of_movement_m, max_authorized_speed_kph, status, issued_at, expires_at";

pub struct AuthorityRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> AuthorityRepository<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    /// Grant a new authority. Any prior `granted`/`active` authority for
    /// the same train is transitioned to `superseded` in the same
    /// transaction — a train may hold at most one live authority, and
    /// this is where that invariant is actually enforced (see the note
    /// in the `train_schema` migration).
    pub async fn grant(
        &self,
        train_id: Uuid,
        block_sequence: Vec<Uuid>,
        limit_of_movement_m: f64,
        max_authorized_speed_kph: i32,
        ttl_secs: i64,
    ) -> AppResult<AuthorityRow> {
        let mut tx = self.pool.begin().await?;

        // Lock the train row first so two concurrent grant requests for
        // the same train can't both proceed to see "no active authority".
        sqlx::query("SELECT id FROM train.trains WHERE id = $1 FOR UPDATE")
            .bind(train_id)
            .fetch_optional(&mut *tx)
            .await?;

        sqlx::query(
            "UPDATE train.movement_authorities SET status = 'superseded' WHERE train_id = $1 AND status IN ('granted', 'active')",
        )
        .bind(train_id)
        .execute(&mut *tx)
        .await?;

        let query = format!(
            r#"
            INSERT INTO train.movement_authorities
                (train_id, block_sequence, limit_of_movement_m, max_authorized_speed_kph, status, expires_at)
            VALUES ($1, $2, $3, $4, 'granted', now() + ($5 || ' seconds')::interval)
            RETURNING {AUTHORITY_COLUMNS}
            "#
        );
        let row: AuthorityRow = sqlx::query_as(&query)
            .bind(train_id)
            .bind(&block_sequence)
            .bind(limit_of_movement_m)
            .bind(max_authorized_speed_kph)
            .bind(ttl_secs.to_string())
            .fetch_one(&mut *tx)
            .await?;

        sqlx::query("UPDATE train.trains SET active_authority = $2 WHERE id = $1")
            .bind(train_id)
            .bind(row.id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(row)
    }

    /// Grant a new authority, but ONLY if no other live authority
    /// already covers any block in `block_sequence` — checked and
    /// enforced inside the same transaction as the grant, not as a
    /// separate pre-check by the caller.
    ///
    /// This exists because the naive flow (interlocking-service reads
    /// current occupancy/authorities, decides "clear", then calls this
    /// service to write) has a real TOCTOU race: two concurrent
    /// requests for two different trains, each wanting an overlapping
    /// but not-yet-granted block, can both pass interlocking's read-side
    /// check before either has written anything. `grant()` above is
    /// therefore unsafe to call from a caller that has only done a
    /// prior read-side check under concurrent load; `interlocking-service`
    /// now calls this method instead, and its own occupancy/authority
    /// check becomes a cheap fast-path rejection, not the safety
    /// boundary — this transaction is the safety boundary.
    ///
    /// Concurrency protection: acquire a Postgres advisory transaction
    /// lock per block in the candidate route, in a fixed (sorted) order
    /// across all callers, before checking or writing anything. Any
    /// second transaction wanting an overlapping block blocks on
    /// `pg_advisory_xact_lock` until the first commits or rolls back —
    /// this is real mutual exclusion, not a "hope nothing interleaves"
    /// check-then-write. Sorting the lock-acquisition order by block ID
    /// (rather than route order) is what prevents two routes that share
    /// two blocks in opposite traversal order from deadlocking each
    /// other.
    pub async fn grant_if_no_conflict(
        &self,
        train_id: Uuid,
        block_sequence: Vec<Uuid>,
        limit_of_movement_m: f64,
        max_authorized_speed_kph: i32,
        ttl_secs: i64,
    ) -> AppResult<Result<AuthorityRow, Vec<Uuid>>> {
        let mut tx = self.pool.begin().await?;

        let mut sorted_blocks = block_sequence.clone();
        sorted_blocks.sort();
        sorted_blocks.dedup();
        for block_id in &sorted_blocks {
            sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
                .bind(block_id.to_string())
                .execute(&mut *tx)
                .await?;
        }

        // Now that every block in the candidate route is locked against
        // concurrent grant attempts, re-check for overlap with any
        // OTHER train's live authority. This check is safe from races
        // now: no other transaction can be concurrently granting an
        // authority over any of these same blocks — it would be
        // blocked on the advisory lock above until this transaction
        // commits or rolls back.
        let conflicting: Vec<Uuid> = sqlx::query_scalar(
            r#"
            SELECT DISTINCT unnest(block_sequence)
            FROM train.movement_authorities
            WHERE status IN ('granted', 'active')
              AND train_id != $1
              AND block_sequence && $2
            "#,
        )
        .bind(train_id)
        .bind(&sorted_blocks)
        .fetch_all(&mut *tx)
        .await?;

        if !conflicting.is_empty() {
            tx.rollback().await?;
            return Ok(Err(conflicting));
        }

        sqlx::query("SELECT id FROM train.trains WHERE id = $1 FOR UPDATE")
            .bind(train_id)
            .fetch_optional(&mut *tx)
            .await?;

        sqlx::query(
            "UPDATE train.movement_authorities SET status = 'superseded' WHERE train_id = $1 AND status IN ('granted', 'active')",
        )
        .bind(train_id)
        .execute(&mut *tx)
        .await?;

        let query = format!(
            r#"
            INSERT INTO train.movement_authorities
                (train_id, block_sequence, limit_of_movement_m, max_authorized_speed_kph, status, expires_at)
            VALUES ($1, $2, $3, $4, 'granted', now() + ($5 || ' seconds')::interval)
            RETURNING {AUTHORITY_COLUMNS}
            "#
        );
        let row: AuthorityRow = sqlx::query_as(&query)
            .bind(train_id)
            .bind(&block_sequence)
            .bind(limit_of_movement_m)
            .bind(max_authorized_speed_kph)
            .bind(ttl_secs.to_string())
            .fetch_one(&mut *tx)
            .await?;

        sqlx::query("UPDATE train.trains SET active_authority = $2 WHERE id = $1")
            .bind(train_id)
            .bind(row.id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(Ok(row))
    }

    pub async fn find_by_id(&self, id: Uuid) -> AppResult<Option<AuthorityRow>> {
        let query =
            format!("SELECT {AUTHORITY_COLUMNS} FROM train.movement_authorities WHERE id = $1");
        Ok(sqlx::query_as(&query)
            .bind(id)
            .fetch_optional(self.pool)
            .await?)
    }

    pub async fn find_active_for_train(&self, train_id: Uuid) -> AppResult<Option<AuthorityRow>> {
        let query = format!(
            "SELECT {AUTHORITY_COLUMNS} FROM train.movement_authorities WHERE train_id = $1 AND status IN ('granted', 'active') ORDER BY issued_at DESC LIMIT 1"
        );
        Ok(sqlx::query_as(&query)
            .bind(train_id)
            .fetch_optional(self.pool)
            .await?)
    }

    pub async fn list_all(&self) -> AppResult<Vec<AuthorityRow>> {
        let query = format!("SELECT {AUTHORITY_COLUMNS} FROM train.movement_authorities ORDER BY issued_at DESC LIMIT 200");
        Ok(sqlx::query_as(&query).fetch_all(self.pool).await?)
    }

    pub async fn set_status(&self, id: Uuid, status: AuthorityStatus) -> AppResult<()> {
        sqlx::query("UPDATE train.movement_authorities SET status = $2 WHERE id = $1")
            .bind(id)
            .bind(status)
            .execute(self.pool)
            .await?;
        Ok(())
    }

    /// Returns authorities that are `granted`/`active` but past their
    /// `expires_at`, for the background expiry sweep.
    pub async fn find_expired(&self) -> AppResult<Vec<AuthorityRow>> {
        let query = format!(
            "SELECT {AUTHORITY_COLUMNS} FROM train.movement_authorities WHERE status IN ('granted', 'active') AND expires_at < now()"
        );
        Ok(sqlx::query_as(&query).fetch_all(self.pool).await?)
    }
}
