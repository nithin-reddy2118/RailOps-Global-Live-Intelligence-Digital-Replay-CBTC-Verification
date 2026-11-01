use cbtc_domain::{BlockId, MovementAuthorityId, SwitchPosition, TrainId};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct RequestAuthorityRequest {
    #[schema(value_type = String)]
    pub train_id: TrainId,
    #[schema(value_type = String)]
    pub destination_block: BlockId,
    #[validate(range(min = 1, max = 400))]
    pub max_authorized_speed_kph: i32,
    #[validate(range(min = 1, max = 3600))]
    pub ttl_secs: i64,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "outcome")]
pub enum RequestAuthorityResponse {
    Granted {
        #[schema(value_type = String)]
        authority_id: MovementAuthorityId,
        #[schema(value_type = Vec<String>)]
        block_sequence: Vec<BlockId>,
    },
    Rejected {
        reasons: Vec<String>,
    },
    NoPathFound,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct SwitchCommandRequest {
    #[schema(value_type = String)]
    pub position: SwitchPosition,
}
