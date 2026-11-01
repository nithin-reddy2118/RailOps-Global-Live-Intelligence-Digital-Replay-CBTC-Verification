use cbtc_domain::{BlockId, MovementAuthorityId, TrainId, TrainOperationalState};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct CreateTrainRequest {
    #[validate(length(min = 1, max = 64))]
    pub identifier: String,
    #[validate(range(min = 1, max = 400))]
    pub max_speed_kph: i32,
    #[validate(range(min = 1.0))]
    pub length_m: f64,
    #[schema(value_type = Option<String>)]
    pub initial_block: Option<BlockId>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TrainResponse {
    #[schema(value_type = String)]
    pub id: TrainId,
    pub identifier: String,
    #[schema(value_type = String)]
    pub operational_state: TrainOperationalState,
    #[schema(value_type = Option<String>)]
    pub current_block: Option<BlockId>,
    #[schema(value_type = Option<String>)]
    pub active_authority: Option<MovementAuthorityId>,
    pub max_speed_kph: i32,
    pub length_m: f64,
    pub last_heartbeat: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct PositionReportRequest {
    #[schema(value_type = String)]
    pub occupied_block: BlockId,
    #[validate(range(min = 0.0))]
    pub offset_within_block_m: f64,
    #[validate(range(min = 0.0, max = 400.0))]
    pub speed_kph: f64,
    pub acceleration_mps2: f64,
    /// The train's own clock reading when it captured this position —
    /// distinct from server arrival time, and required to detect
    /// stale/out-of-order delivery (see
    /// `repository::TrainRepository::record_position_report`).
    pub client_timestamp: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct StateChangeRequest {
    #[schema(value_type = String)]
    pub new_state: TrainOperationalState,
    #[validate(length(min = 1))]
    pub reason: String,
}
