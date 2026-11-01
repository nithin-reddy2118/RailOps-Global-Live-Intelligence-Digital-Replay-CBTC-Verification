//! Shared integration-test harness. Every service's `tests/` directory
//! depends on this crate rather than re-implementing pool setup and
//! fixture builders.
//!
//! Tests are expected to run against a real Postgres instance reachable
//! via `TEST_DATABASE_URL` (see each service's `tests/` for the exact
//! docker-compose service used in CI). Each test gets an isolated
//! schema so tests can run concurrently without clobbering each other's
//! data.

use cbtc_domain::{
    AuthorityStatus, BlockId, MovementAuthority, MovementAuthorityId, OccupancyState, SignalAspect,
    SignalId, SignalKind, TrackBlock, Train, TrainId, TrainOperationalState,
};
use chrono::Utc;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use uuid::Uuid;

/// Connect to the test database named by `TEST_DATABASE_URL`, create a
/// uniquely-named schema, run migrations into it, and return a pool
/// scoped (via `search_path`) to that schema. The schema is left in
/// place after the test for post-mortem debugging; CI periodically
/// reaps schemas older than 1 hour rather than dropping them per-test,
/// so failed-test state remains inspectable.
pub async fn test_pool(migrations_path: &str) -> PgPool {
    let base_url = std::env::var("TEST_DATABASE_URL")
        .expect("TEST_DATABASE_URL must be set to run integration tests");

    let admin_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&base_url)
        .await
        .expect("failed to connect to test database");

    let schema_name = format!("test_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE SCHEMA \"{schema_name}\""))
        .execute(&admin_pool)
        .await
        .expect("failed to create test schema");

    let scoped_url = format!("{base_url}?options=-csearch_path%3D{schema_name}");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&scoped_url)
        .await
        .expect("failed to connect with schema search_path");

    sqlx::migrate::Migrator::new(std::path::Path::new(migrations_path))
        .await
        .expect("failed to load migrator")
        .run(&pool)
        .await
        .expect("failed to run migrations against test schema");

    pool
}

/// A minimal, valid `Train` fixture in the `Staged` state, ready to be
/// inserted or used directly in unit tests that don't touch the DB.
pub fn fixture_train() -> Train {
    Train {
        id: TrainId::new(),
        identifier: format!("T-{}", &Uuid::new_v4().simple().to_string()[..6]),
        operational_state: TrainOperationalState::Staged,
        current_block: None,
        active_authority: None,
        max_speed_kph: 80,
        length_m: 90.0,
        last_position_report: None,
        last_heartbeat: Utc::now(),
    }
}

pub fn fixture_clear_block() -> TrackBlock {
    TrackBlock {
        id: BlockId::new(),
        name: format!("BLK-{}", &Uuid::new_v4().simple().to_string()[..6]),
        length_m: 250.0,
        occupancy: OccupancyState::Clear,
        occupied_by: None,
        last_updated: Utc::now(),
        max_speed_kph: 80,
        bidirectional: false,
    }
}

pub fn fixture_signal(protects: BlockId) -> cbtc_domain::Signal {
    cbtc_domain::Signal {
        id: SignalId::new(),
        name: format!("SIG-{}", &Uuid::new_v4().simple().to_string()[..6]),
        kind: SignalKind::Main,
        protects_block: protects,
        current_aspect: SignalAspect::Red,
        last_aspect_change: Utc::now(),
        chainage_m: 0.0,
    }
}

pub fn fixture_authority(train_id: TrainId, blocks: Vec<BlockId>) -> MovementAuthority {
    let now = Utc::now();
    MovementAuthority {
        id: MovementAuthorityId::new(),
        train_id,
        route_id: None,
        block_sequence: blocks,
        limit_of_movement_m: 200.0,
        max_authorized_speed_kph: 60,
        status: AuthorityStatus::Granted,
        issued_at: now,
        expires_at: now + chrono::Duration::minutes(10),
    }
}
