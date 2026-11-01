//! Track block (the atomic unit of occupancy detection) types.

use crate::ids::{BlockId, TrainId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Occupancy state of a track block as reported by wayside detection
/// (axle counters / track circuits in the real world; simulated sensors
/// here). `Ambiguous` means detection hardware disagrees or has not
/// reported within the expected heartbeat window — it must be treated
/// as occupied by any safety-relevant consumer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "occupancy_state", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum OccupancyState {
    Clear,
    Occupied,
    Ambiguous,
    OutOfService,
}

impl OccupancyState {
    /// Whether this state must be treated as unsafe-to-enter by
    /// interlocking logic. `Clear` is the only state that permits
    /// granting a new movement authority through this block.
    pub fn is_safe_to_enter(&self) -> bool {
        matches!(self, OccupancyState::Clear)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackBlock {
    pub id: BlockId,
    pub name: String,
    pub length_m: f64,
    pub occupancy: OccupancyState,
    /// The train currently believed to occupy this block, if any.
    /// `None` while `occupancy == Occupied` is a valid transient state
    /// (e.g. an unidentified occupancy from a track circuit before the
    /// train report correlates it) and must be surfaced as a degraded
    /// condition, not silently ignored.
    pub occupied_by: Option<TrainId>,
    pub last_updated: DateTime<Utc>,
    /// Maximum permitted speed on this block in km/h under normal
    /// conditions (civil speed restriction, not authority-limited speed).
    pub max_speed_kph: u32,
    pub bidirectional: bool,
}
