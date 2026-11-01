use cbtc_domain::{BlockId, OccupancyState, TrainId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

#[derive(Debug, Serialize, ToSchema)]
pub struct BlockResponse {
    #[schema(value_type = String)]
    pub id: BlockId,
    pub name: String,
    pub length_m: f64,
    #[schema(value_type = String)]
    pub occupancy: OccupancyState,
    #[schema(value_type = Option<String>)]
    pub occupied_by: Option<TrainId>,
    pub last_updated: DateTime<Utc>,
    pub max_speed_kph: i32,
    pub bidirectional: bool,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct ManualOccupancyRequest {
    #[schema(value_type = String)]
    pub occupancy: OccupancyState,
    #[validate(length(min = 1))]
    pub reason: String,
}
