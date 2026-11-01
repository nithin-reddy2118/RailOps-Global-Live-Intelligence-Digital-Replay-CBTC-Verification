use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema)]
pub struct AuditEntryResponse {
    pub id: i64,
    pub event_id: Uuid,
    pub causality_id: Uuid,
    pub origin_service: String,
    pub actor_user_id: Option<Uuid>,
    pub action: String,
    pub details: Value,
    pub occurred_at: DateTime<Utc>,
}
