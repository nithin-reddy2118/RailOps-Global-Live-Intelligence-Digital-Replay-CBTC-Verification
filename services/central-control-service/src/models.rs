use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct TrainOverview {
    pub id: Uuid,
    pub identifier: String,
    pub operational_state: String,
    pub current_block: Option<Uuid>,
    pub last_heartbeat: DateTime<Utc>,
}

#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct BlockOverview {
    pub id: Uuid,
    pub name: String,
    pub occupancy: String,
    pub occupied_by: Option<Uuid>,
}

#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct SignalOverview {
    pub id: Uuid,
    pub name: String,
    pub current_aspect: String,
    pub protects_block: Uuid,
}

#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct SwitchOverview {
    pub id: Uuid,
    pub name: String,
    pub position: String,
    pub locked: bool,
}

#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct AuthorityOverview {
    pub id: Uuid,
    pub train_id: Uuid,
    pub status: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct UnacknowledgedAlertOverview {
    pub id: Uuid,
    pub severity: String,
    pub title: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NetworkOverview {
    pub trains: Vec<TrainOverview>,
    pub blocks: Vec<BlockOverview>,
    pub signals: Vec<SignalOverview>,
    pub switches: Vec<SwitchOverview>,
    pub active_authorities: Vec<AuthorityOverview>,
    pub unacknowledged_alerts: Vec<UnacknowledgedAlertOverview>,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct EmergencyStopResult {
    pub authorities_revoked: usize,
    pub failures: Vec<String>,
}
