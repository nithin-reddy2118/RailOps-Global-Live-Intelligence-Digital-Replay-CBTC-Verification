use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct PositionReportRequest {
    pub occupied_block: Uuid,
    pub offset_within_block_m: f64,
    pub speed_kph: f64,
    pub acceleration_mps2: f64,
    pub client_timestamp: DateTime<Utc>,
}
