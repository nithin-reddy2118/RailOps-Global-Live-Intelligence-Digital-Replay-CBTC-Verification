//! Signal aspect computation. Kept pure and side-effect-free so it can
//! be unit tested without a database — mirrors the approach taken in
//! `interlocking-service/src/safety.rs`.
//!
//! Aspect rule (standard two-block lookahead, single-yellow variant of
//! multi-aspect signaling):
//!   - `protects_block` not clear                       -> Red
//!   - `protects_block` clear, next block not clear      -> YellowSingle
//!   - `protects_block` clear, next block clear (or none) -> Green
//!   - `protects_block` state unknown (not in map)        -> Red (fail-safe)

use cbtc_domain::{OccupancyState, SignalAspect};

pub fn compute_aspect(
    protects_block: Option<OccupancyState>,
    next_block: Option<OccupancyState>,
) -> SignalAspect {
    match protects_block {
        None => SignalAspect::Red, // fail-safe: unknown state is never permissive
        Some(state) if state != OccupancyState::Clear => SignalAspect::Red,
        Some(_) => match next_block {
            None => SignalAspect::Green, // no further block to check (end of line / no adjacency)
            Some(OccupancyState::Clear) => SignalAspect::Green,
            Some(_) => SignalAspect::YellowSingle,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protected_block_occupied_is_always_red() {
        assert_eq!(
            compute_aspect(Some(OccupancyState::Occupied), Some(OccupancyState::Clear)),
            SignalAspect::Red
        );
    }

    #[test]
    fn unknown_protected_block_is_fail_safe_red() {
        assert_eq!(
            compute_aspect(None, Some(OccupancyState::Clear)),
            SignalAspect::Red
        );
    }

    #[test]
    fn clear_ahead_and_next_clear_is_green() {
        assert_eq!(
            compute_aspect(Some(OccupancyState::Clear), Some(OccupancyState::Clear)),
            SignalAspect::Green
        );
    }

    #[test]
    fn clear_ahead_but_next_occupied_is_single_yellow() {
        assert_eq!(
            compute_aspect(Some(OccupancyState::Clear), Some(OccupancyState::Occupied)),
            SignalAspect::YellowSingle
        );
    }

    #[test]
    fn clear_ahead_with_no_next_block_is_green() {
        assert_eq!(
            compute_aspect(Some(OccupancyState::Clear), None),
            SignalAspect::Green
        );
    }

    #[test]
    fn ambiguous_protected_block_is_red_not_permissive() {
        assert_eq!(
            compute_aspect(Some(OccupancyState::Ambiguous), Some(OccupancyState::Clear)),
            SignalAspect::Red
        );
    }

    #[test]
    fn out_of_service_protected_block_is_red() {
        assert_eq!(
            compute_aspect(
                Some(OccupancyState::OutOfService),
                Some(OccupancyState::Clear)
            ),
            SignalAspect::Red
        );
    }
}
