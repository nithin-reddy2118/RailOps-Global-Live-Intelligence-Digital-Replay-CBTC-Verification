//! Station and platform types, used by Passenger Information Service and
//! the dashboard map.

use crate::ids::{BlockId, PlatformId, StationId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Platform {
    pub id: PlatformId,
    pub station_id: StationId,
    pub name: String,
    pub adjacent_block: BlockId,
    pub dwell_time_default_secs: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Station {
    pub id: StationId,
    pub name: String,
    pub chainage_m: f64,
    pub platforms: Vec<Platform>,
}
