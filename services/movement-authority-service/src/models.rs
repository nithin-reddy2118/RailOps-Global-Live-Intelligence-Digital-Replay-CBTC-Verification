use cbtc_domain::{AuthorityStatus, BlockId, MovementAuthorityId, TrainId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

/// Grant request body. This endpoint is called by interlocking-service
/// after it has already performed conflict/safety validation — this
/// service does not itself decide whether a grant is safe, only records
/// the decision and manages its lifecycle (expiry, revocation).
#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct GrantAuthorityRequest {
    #[schema(value_type = String)]
    pub train_id: TrainId,
    #[schema(value_type = Vec<String>)]
    #[validate(length(min = 1, message = "block_sequence must contain at least one block"))]
    pub block_sequence: Vec<BlockId>,
    #[validate(range(min = 0.0))]
    pub limit_of_movement_m: f64,
    #[validate(range(min = 1, max = 400))]
    pub max_authorized_speed_kph: i32,
    #[validate(range(min = 1, max = 3600))]
    pub ttl_secs: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AuthorityResponse {
    #[schema(value_type = String)]
    pub id: MovementAuthorityId,
    #[schema(value_type = String)]
    pub train_id: TrainId,
    #[schema(value_type = Vec<String>)]
    pub block_sequence: Vec<BlockId>,
    pub limit_of_movement_m: f64,
    pub max_authorized_speed_kph: i32,
    #[schema(value_type = String)]
    pub status: AuthorityStatus,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct RevokeAuthorityRequest {
    #[validate(length(min = 1))]
    pub reason: String,
}
