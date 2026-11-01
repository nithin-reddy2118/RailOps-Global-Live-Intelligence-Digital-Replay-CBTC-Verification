//! Movement Authority (MA) types — the core CBTC safety primitive.
//!
//! A movement authority is a time-and-distance-bounded permission for a
//! specific train to occupy a specific sequence of blocks up to a limit
//! of movement (LOM). No train may advance beyond its LOM without a new
//! or extended authority; enforcing that is the interlocking service's
//! job, but the type lives here since Train, Block Occupancy, and
//! Central Control all need to reason about it too.

use crate::ids::{BlockId, MovementAuthorityId, RouteId, TrainId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "authority_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum AuthorityStatus {
    Requested,
    Granted,
    Active,
    Expired,
    Revoked,
    Superseded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MovementAuthority {
    pub id: MovementAuthorityId,
    pub train_id: TrainId,
    pub route_id: Option<RouteId>,
    /// Ordered sequence of blocks this authority covers, from the
    /// train's current block (inclusive) to the limit of movement.
    pub block_sequence: Vec<BlockId>,
    /// Limit of movement: distance in meters from the start of the last
    /// block in `block_sequence` at which the train must be stopped or
    /// re-authorized.
    pub limit_of_movement_m: f64,
    pub max_authorized_speed_kph: u32,
    pub status: AuthorityStatus,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

impl MovementAuthority {
    pub fn is_valid_at(&self, now: DateTime<Utc>) -> bool {
        matches!(
            self.status,
            AuthorityStatus::Granted | AuthorityStatus::Active
        ) && now < self.expires_at
    }

    /// The terminal block covered by this authority — the block the
    /// limit of movement falls within.
    pub fn terminal_block(&self) -> Option<BlockId> {
        self.block_sequence.last().copied()
    }
}
