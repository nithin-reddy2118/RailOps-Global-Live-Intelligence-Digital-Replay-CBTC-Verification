use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema)]
pub struct PlatformSummary {
    pub id: Uuid,
    pub name: String,
    pub adjacent_block: Uuid,
    pub dwell_time_default_secs: i32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct StationResponse {
    pub id: Uuid,
    pub name: String,
    pub chainage_m: f64,
    pub platforms: Vec<PlatformSummary>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "status")]
pub enum PlatformStatus {
    TrainAtPlatform {
        train_id: Uuid,
        train_identifier: String,
    },
    TrainApproaching {
        train_id: Uuid,
        train_identifier: String,
        estimated_seconds_away: Option<f64>,
    },
    NoTrainExpected,
}
