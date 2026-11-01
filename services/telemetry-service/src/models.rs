use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct PositionReportPoint {
    pub occupied_block: Uuid,
    pub offset_within_block_m: f64,
    pub speed_kph: f64,
    pub acceleration_mps2: f64,
    pub reported_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TrainTelemetrySummary {
    pub train_id: Uuid,
    pub report_count: i64,
    pub average_speed_kph: Option<f64>,
    pub max_speed_kph: Option<f64>,
    pub window_start: Option<DateTime<Utc>>,
    pub window_end: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct HistoryQuery {
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_limit() -> i64 {
    500
}
