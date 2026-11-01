//! Track switch (points) types.

use crate::ids::{BlockId, SwitchId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "switch_position", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum SwitchPosition {
    Normal,
    Reverse,
    /// Points are moving between positions, or detection disagrees with
    /// the commanded position. Must be treated as unsafe to route through.
    InTransition,
    Failed,
}

impl SwitchPosition {
    pub fn is_locked_and_detected(&self) -> bool {
        matches!(self, SwitchPosition::Normal | SwitchPosition::Reverse)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Switch {
    pub id: SwitchId,
    pub name: String,
    pub position: SwitchPosition,
    /// The block the switch sits within / controls entry to.
    pub host_block: BlockId,
    /// Block reached when the switch is in the `Normal` position.
    pub normal_route_block: BlockId,
    /// Block reached when the switch is in the `Reverse` position.
    pub reverse_route_block: BlockId,
    pub last_moved_at: DateTime<Utc>,
    /// True while a route lock (from interlocking-service) prevents this
    /// switch from being commanded to move.
    pub locked: bool,
}
