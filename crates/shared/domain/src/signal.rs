//! Wayside and cab signaling types.

use crate::ids::{BlockId, SignalId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// The aspect (displayed state) of a signal.
///
/// Modeled after standard multi-aspect railway signaling. `Failed`
/// represents a detected sensor/hardware fault and must always be
/// treated as equivalent to or more restrictive than `Red` by any
/// consumer — enforced in interlocking-service, not here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "signal_aspect", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum SignalAspect {
    Red,
    YellowSingle,
    YellowDouble,
    Green,
    Failed,
    Unknown,
}

impl SignalAspect {
    /// Whether a train may proceed past a signal showing this aspect
    /// under normal (non-degraded) operating rules.
    pub fn permits_proceed(&self) -> bool {
        matches!(
            self,
            SignalAspect::Green | SignalAspect::YellowSingle | SignalAspect::YellowDouble
        )
    }

    pub fn is_restrictive(&self) -> bool {
        matches!(
            self,
            SignalAspect::Red | SignalAspect::Failed | SignalAspect::Unknown
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "signal_kind", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum SignalKind {
    Main,
    Shunt,
    Repeater,
    Distant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signal {
    pub id: SignalId,
    pub name: String,
    pub kind: SignalKind,
    pub protects_block: BlockId,
    pub current_aspect: SignalAspect,
    pub last_aspect_change: DateTime<Utc>,
    /// Chainage (track-km position) used for map rendering and distance
    /// calculations; unit is meters from the line's zero reference point.
    pub chainage_m: f64,
}
