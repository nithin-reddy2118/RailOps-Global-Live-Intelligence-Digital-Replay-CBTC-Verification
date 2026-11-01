//! Train fleet and telemetry types.

use crate::ids::{BlockId, MovementAuthorityId, TrainId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "train_operational_state", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum TrainOperationalState {
    OutOfService,
    Staged,
    InMotion,
    Stopped,
    EmergencyBraking,
    Degraded,
    CommunicationLost,
}

impl TrainOperationalState {
    /// States in which the train must not be granted a new movement
    /// authority until it transitions to a safe state.
    pub fn blocks_new_authority(&self) -> bool {
        matches!(
            self,
            TrainOperationalState::EmergencyBraking
                | TrainOperationalState::Degraded
                | TrainOperationalState::CommunicationLost
                | TrainOperationalState::OutOfService
        )
    }
}

/// A train's live kinematic report, as periodically transmitted over the
/// train-to-wayside communication link (simulated here, real ATP/ATO
/// telemetry in a production system).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainPositionReport {
    pub train_id: TrainId,
    pub occupied_block: BlockId,
    /// Offset in meters from the start of `occupied_block`, in the
    /// direction of increasing chainage.
    pub offset_within_block_m: f64,
    pub speed_kph: f64,
    /// Positive = accelerating, negative = decelerating, in m/s^2.
    pub acceleration_mps2: f64,
    pub reported_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Train {
    pub id: TrainId,
    pub identifier: String,
    pub operational_state: TrainOperationalState,
    pub current_block: Option<BlockId>,
    pub active_authority: Option<MovementAuthorityId>,
    pub max_speed_kph: u32,
    pub length_m: f64,
    pub last_position_report: Option<TrainPositionReport>,
    pub last_heartbeat: DateTime<Utc>,
}

impl Train {
    /// A train is considered to have lost communication if no heartbeat
    /// has been received within this window. Central Control Service and
    /// Interlocking Service both key their degraded-mode logic off this
    /// constant so their behavior can never silently diverge.
    pub const HEARTBEAT_TIMEOUT_SECS: i64 = 5;

    pub fn has_lost_communication(&self, now: DateTime<Utc>) -> bool {
        (now - self.last_heartbeat).num_seconds() > Self::HEARTBEAT_TIMEOUT_SECS
    }
}
