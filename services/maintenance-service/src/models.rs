use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[sqlx(type_name = "maintenance_ticket_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum TicketStatus {
    Open,
    InProgress,
    Resolved,
    Closed,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct OpenTicketRequest {
    #[validate(length(min = 1, max = 256))]
    pub asset_description: String,
    pub related_block: Option<Uuid>,
    pub related_switch: Option<Uuid>,
    pub related_signal: Option<Uuid>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct UpdateTicketRequest {
    pub status: TicketStatus,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct TicketResponse {
    pub id: Uuid,
    pub asset_description: String,
    pub related_block: Option<Uuid>,
    pub related_switch: Option<Uuid>,
    pub related_signal: Option<Uuid>,
    pub status: TicketStatus,
    pub opened_by: Uuid,
    pub opened_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
}
