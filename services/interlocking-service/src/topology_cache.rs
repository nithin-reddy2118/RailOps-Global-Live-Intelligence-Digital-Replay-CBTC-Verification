//! In-memory topology + live switch-position cache.
//!
//! Rebuilt from the database at startup and refreshed incrementally as
//! `SwitchPositionChanged` events arrive over MQ (see `mq_consumer.rs`).
//! Route computation reads this cache rather than hitting the database
//! per request, since pathfinding may need to evaluate many candidate
//! paths per authority request.

use cbtc_domain::{BlockAdjacency, SwitchId, SwitchPosition, TrackTopology};
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::RwLock;
use uuid::Uuid;

pub struct TopologyCache {
    inner: RwLock<CacheData>,
}

struct CacheData {
    topology: TrackTopology,
    switch_positions: HashMap<SwitchId, SwitchPosition>,
}

impl TopologyCache {
    pub async fn load(pool: &PgPool) -> Result<Self, sqlx::Error> {
        let adjacency_rows: Vec<(Uuid, Uuid, Option<Uuid>, Option<SwitchPosition>)> = sqlx::query_as(
            "SELECT from_block, to_block, via_switch, required_switch_position FROM signaling.block_adjacency",
        )
        .fetch_all(pool)
        .await?;

        let adjacencies = adjacency_rows
            .into_iter()
            .map(|(from, to, via_switch, required)| BlockAdjacency {
                from: from.into(),
                to: to.into(),
                via_switch: via_switch.map(SwitchId::from),
                required_switch_position: required,
            })
            .collect();

        let switch_rows: Vec<(Uuid, SwitchPosition)> =
            sqlx::query_as("SELECT id, position FROM signaling.switches")
                .fetch_all(pool)
                .await?;

        let switch_positions = switch_rows
            .into_iter()
            .map(|(id, pos)| (SwitchId::from(id), pos))
            .collect();

        Ok(Self {
            inner: RwLock::new(CacheData {
                topology: TrackTopology::new(adjacencies),
                switch_positions,
            }),
        })
    }

    pub fn update_switch_position(&self, switch_id: SwitchId, position: SwitchPosition) {
        let mut data = self.inner.write().expect("topology cache lock poisoned");
        data.switch_positions.insert(switch_id, position);
    }

    /// Breadth-first search from `start` to `goal` over the current
    /// topology, honoring live switch positions. Returns the ordered
    /// block sequence including both endpoints, or `None` if no path
    /// exists given the current switch alignment.
    pub fn find_path(
        &self,
        start: cbtc_domain::BlockId,
        goal: cbtc_domain::BlockId,
    ) -> Option<Vec<cbtc_domain::BlockId>> {
        let data = self.inner.read().expect("topology cache lock poisoned");

        if start == goal {
            return Some(vec![start]);
        }

        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        let mut came_from: HashMap<cbtc_domain::BlockId, cbtc_domain::BlockId> = HashMap::new();

        visited.insert(start);
        queue.push_back(start);

        while let Some(current) = queue.pop_front() {
            if current == goal {
                let mut path = vec![goal];
                let mut node = goal;
                while let Some(&prev) = came_from.get(&node) {
                    path.push(prev);
                    node = prev;
                }
                path.reverse();
                return Some(path);
            }

            for next in data
                .topology
                .reachable_from(current, &data.switch_positions)
            {
                if visited.insert(next) {
                    came_from.insert(next, current);
                    queue.push_back(next);
                }
            }
        }

        None
    }
}
