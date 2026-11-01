//! Core safety logic. Every function here is a pure conflict *check* —
//! none of them mutate state. The handler that calls these is
//! responsible for acting on the result (granting, rejecting, or
//! revoking). Keeping the checks pure and side-effect-free makes them
//! independently unit-testable without a database.

use cbtc_domain::{BlockId, OccupancyState, SwitchPosition};
use std::collections::HashMap;

#[derive(Debug, PartialEq, Eq)]
pub enum ConflictReason {
    BlockNotClear {
        block: BlockId,
        state: OccupancyState,
    },
    SwitchNotLockedAndDetected {
        block: BlockId,
    },
    OverlapsExistingAuthority {
        block: BlockId,
    },
}

impl std::fmt::Display for ConflictReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConflictReason::BlockNotClear { block, state } => {
                write!(f, "block {block} is not clear (state: {state:?})")
            }
            ConflictReason::SwitchNotLockedAndDetected { block } => {
                write!(
                    f,
                    "a switch controlling entry to block {block} is not locked and detected"
                )
            }
            ConflictReason::OverlapsExistingAuthority { block } => {
                write!(
                    f,
                    "block {block} is already covered by another train's active authority"
                )
            }
        }
    }
}

/// Validate that every block in `requested_path` (excluding the train's
/// current block, which it already legitimately occupies) is clear, and
/// that no other train already holds a live authority over any of those
/// blocks.
///
/// `occupancy` and `existing_authority_blocks` are supplied by the
/// caller (queried fresh from the database immediately before this
/// check) rather than read from any cache, since occupancy and existing
/// authorities are exactly the kind of fast-changing, safety-critical
/// state that must never be served stale.
pub fn check_path_conflicts(
    requesting_train_current_block: BlockId,
    requested_path: &[BlockId],
    occupancy: &HashMap<BlockId, OccupancyState>,
    existing_authority_blocks: &std::collections::HashSet<BlockId>,
) -> Vec<ConflictReason> {
    let mut conflicts = Vec::new();

    for &block in requested_path {
        if block == requesting_train_current_block {
            continue;
        }

        match occupancy.get(&block) {
            Some(OccupancyState::Clear) => {}
            Some(state) => conflicts.push(ConflictReason::BlockNotClear {
                block,
                state: *state,
            }),
            None => conflicts.push(ConflictReason::BlockNotClear {
                block,
                state: OccupancyState::Ambiguous,
            }),
        }

        if existing_authority_blocks.contains(&block) {
            conflicts.push(ConflictReason::OverlapsExistingAuthority { block });
        }
    }

    conflicts
}

/// Validate that every switch gating entry to a block in the path is in
/// a locked-and-detected position (`Normal` or `Reverse`, never
/// `InTransition` or `Failed`).
pub fn check_switch_states(switches_in_path: &[(BlockId, SwitchPosition)]) -> Vec<ConflictReason> {
    switches_in_path
        .iter()
        .filter(|(_, pos)| !pos.is_locked_and_detected())
        .map(|(block, _)| ConflictReason::SwitchNotLockedAndDetected { block: *block })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn block(n: u8) -> BlockId {
        BlockId::from(uuid::Uuid::from_bytes([n; 16]))
    }

    #[test]
    fn clear_path_with_no_overlap_has_no_conflicts() {
        let mut occupancy = HashMap::new();
        occupancy.insert(block(2), OccupancyState::Clear);
        occupancy.insert(block(3), OccupancyState::Clear);

        let conflicts =
            check_path_conflicts(block(1), &[block(2), block(3)], &occupancy, &HashSet::new());
        assert!(conflicts.is_empty());
    }

    #[test]
    fn occupied_block_is_flagged() {
        let mut occupancy = HashMap::new();
        occupancy.insert(block(2), OccupancyState::Occupied);

        let conflicts = check_path_conflicts(block(1), &[block(2)], &occupancy, &HashSet::new());
        assert_eq!(conflicts.len(), 1);
        assert!(matches!(conflicts[0], ConflictReason::BlockNotClear { .. }));
    }

    #[test]
    fn unknown_block_occupancy_is_treated_as_ambiguous_not_clear() {
        // Fail-safe: a block missing from the occupancy map (e.g. a
        // caller bug, or a block deleted mid-flight) must never be
        // treated as implicitly clear.
        let occupancy = HashMap::new();
        let conflicts = check_path_conflicts(block(1), &[block(2)], &occupancy, &HashSet::new());
        assert_eq!(conflicts.len(), 1);
    }

    #[test]
    fn overlapping_existing_authority_is_flagged_even_if_clear() {
        let mut occupancy = HashMap::new();
        occupancy.insert(block(2), OccupancyState::Clear);
        let mut existing = HashSet::new();
        existing.insert(block(2));

        let conflicts = check_path_conflicts(block(1), &[block(2)], &occupancy, &existing);
        assert_eq!(conflicts.len(), 1);
        assert!(matches!(
            conflicts[0],
            ConflictReason::OverlapsExistingAuthority { .. }
        ));
    }

    #[test]
    fn train_current_block_is_exempt_from_the_clear_check() {
        let occupancy = HashMap::new(); // deliberately empty/ambiguous
        let conflicts = check_path_conflicts(block(1), &[block(1)], &occupancy, &HashSet::new());
        assert!(
            conflicts.is_empty(),
            "a train's own current block must not be flagged against itself"
        );
    }

    #[test]
    fn switch_in_transition_is_flagged() {
        let conflicts = check_switch_states(&[(block(2), SwitchPosition::InTransition)]);
        assert_eq!(conflicts.len(), 1);
    }

    #[test]
    fn switch_normal_or_reverse_is_not_flagged() {
        let conflicts = check_switch_states(&[
            (block(2), SwitchPosition::Normal),
            (block(3), SwitchPosition::Reverse),
        ]);
        assert!(conflicts.is_empty());
    }
}
