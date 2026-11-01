//! Track topology — the connectivity graph interlocking-service and
//! movement-authority-service traverse to compute routes and validate
//! that a requested path is physically continuous and conflict-free.

use crate::ids::{BlockId, SwitchId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Directed adjacency between two blocks, optionally gated by a switch
/// that must be in a specific position for the adjacency to be traversable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockAdjacency {
    pub from: BlockId,
    pub to: BlockId,
    pub via_switch: Option<SwitchId>,
    /// If `via_switch` is `Some`, the switch position required for this
    /// adjacency to be usable. Ignored (treated as always traversable)
    /// when `via_switch` is `None`.
    pub required_switch_position: Option<crate::switch::SwitchPosition>,
}

/// In-memory representation of the full track graph for a line. Built
/// once at service startup from the database and refreshed on topology
/// change events; not intended to be mutated per-request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrackTopology {
    pub adjacencies: Vec<BlockAdjacency>,
}

impl TrackTopology {
    pub fn new(adjacencies: Vec<BlockAdjacency>) -> Self {
        Self { adjacencies }
    }

    /// Build an adjacency-list index for fast graph traversal. Recomputed
    /// on demand rather than cached on the struct so `TrackTopology`
    /// remains cheaply serializable for transmission to the dashboard.
    pub fn adjacency_index(&self) -> HashMap<BlockId, Vec<&BlockAdjacency>> {
        let mut index: HashMap<BlockId, Vec<&BlockAdjacency>> = HashMap::new();
        for adj in &self.adjacencies {
            index.entry(adj.from).or_default().push(adj);
        }
        index
    }

    /// Returns the blocks immediately reachable from `block`, given the
    /// current switch positions supplied by the caller (usually sourced
    /// live from block-occupancy-service / wayside state).
    pub fn reachable_from(
        &self,
        block: BlockId,
        switch_positions: &HashMap<SwitchId, crate::switch::SwitchPosition>,
    ) -> Vec<BlockId> {
        self.adjacencies
            .iter()
            .filter(|adj| adj.from == block)
            .filter(
                |adj| match (&adj.via_switch, &adj.required_switch_position) {
                    (Some(switch_id), Some(required)) => {
                        switch_positions.get(switch_id) == Some(required)
                    }
                    _ => true,
                },
            )
            .map(|adj| adj.to)
            .collect()
    }
}
