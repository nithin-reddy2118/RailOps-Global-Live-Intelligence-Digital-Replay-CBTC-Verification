use cbtc_domain::{BlockAdjacency, BlockId, SwitchId, SwitchPosition, TrackTopology};
use std::collections::HashMap;

fn block(n: u8) -> BlockId {
    BlockId::from(uuid::Uuid::from_bytes([n; 16]))
}

fn switch(n: u8) -> SwitchId {
    SwitchId::from(uuid::Uuid::from_bytes([n; 16]))
}

#[test]
fn reachable_from_respects_required_switch_position() {
    let topology = TrackTopology::new(vec![
        BlockAdjacency {
            from: block(1),
            to: block(2),
            via_switch: Some(switch(1)),
            required_switch_position: Some(SwitchPosition::Normal),
        },
        BlockAdjacency {
            from: block(1),
            to: block(3),
            via_switch: Some(switch(1)),
            required_switch_position: Some(SwitchPosition::Reverse),
        },
    ]);

    let mut positions = HashMap::new();
    positions.insert(switch(1), SwitchPosition::Normal);

    let reachable = topology.reachable_from(block(1), &positions);
    assert_eq!(reachable, vec![block(2)]);

    positions.insert(switch(1), SwitchPosition::Reverse);
    let reachable = topology.reachable_from(block(1), &positions);
    assert_eq!(reachable, vec![block(3)]);
}

#[test]
fn unconditional_adjacency_is_always_reachable() {
    let topology = TrackTopology::new(vec![BlockAdjacency {
        from: block(1),
        to: block(2),
        via_switch: None,
        required_switch_position: None,
    }]);

    let reachable = topology.reachable_from(block(1), &HashMap::new());
    assert_eq!(reachable, vec![block(2)]);
}
