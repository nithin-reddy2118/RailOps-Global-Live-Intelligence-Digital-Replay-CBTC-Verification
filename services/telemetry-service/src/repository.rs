use crate::models::{PositionReportPoint, TrainTelemetrySummary};
use cbtc_errors::AppResult;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

pub struct TelemetryRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> TelemetryRepository<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn history(
        &self,
        train_id: Uuid,
        from: Option<DateTime<Utc>>,
        to: Option<DateTime<Utc>>,
        limit: i64,
    ) -> AppResult<Vec<PositionReportPoint>> {
        let from = from.unwrap_or_else(|| Utc::now() - chrono::Duration::hours(1));
        let to = to.unwrap_or_else(Utc::now);

        Ok(sqlx::query_as::<_, PositionReportPoint>(
            r#"
            SELECT occupied_block, offset_within_block_m, speed_kph, acceleration_mps2, reported_at
            FROM train.position_reports
            WHERE train_id = $1 AND reported_at BETWEEN $2 AND $3
            ORDER BY reported_at DESC
            LIMIT $4
            "#,
        )
        .bind(train_id)
        .bind(from)
        .bind(to)
        .bind(limit)
        .fetch_all(self.pool)
        .await?)
    }

    pub async fn latest(&self, train_id: Uuid) -> AppResult<Option<PositionReportPoint>> {
        Ok(sqlx::query_as::<_, PositionReportPoint>(
            r#"
            SELECT occupied_block, offset_within_block_m, speed_kph, acceleration_mps2, reported_at
            FROM train.position_reports
            WHERE train_id = $1
            ORDER BY reported_at DESC
            LIMIT 1
            "#,
        )
        .bind(train_id)
        .fetch_optional(self.pool)
        .await?)
    }

    pub async fn summary(
        &self,
        train_id: Uuid,
        from: Option<DateTime<Utc>>,
        to: Option<DateTime<Utc>>,
    ) -> AppResult<TrainTelemetrySummary> {
        let from = from.unwrap_or_else(|| Utc::now() - chrono::Duration::hours(1));
        let to = to.unwrap_or_else(Utc::now);

        #[allow(clippy::type_complexity)]
        let row: (
            i64,
            Option<f64>,
            Option<f64>,
            Option<DateTime<Utc>>,
            Option<DateTime<Utc>>,
        ) = sqlx::query_as(
            r#"
            SELECT
                COUNT(*),
                AVG(speed_kph),
                MAX(speed_kph),
                MIN(reported_at),
                MAX(reported_at)
            FROM train.position_reports
            WHERE train_id = $1 AND reported_at BETWEEN $2 AND $3
            "#,
        )
        .bind(train_id)
        .bind(from)
        .bind(to)
        .fetch_one(self.pool)
        .await?;

        Ok(TrainTelemetrySummary {
            train_id,
            report_count: row.0,
            average_speed_kph: row.1,
            max_speed_kph: row.2,
            window_start: row.3,
            window_end: row.4,
        })
    }
}
