//! Run with: TEST_DATABASE_URL=postgres://cbtc:cbtc@localhost:5432/cbtc_test cargo test -p block-occupancy-service

use block_occupancy_service::repository::{BlockRepository, MoveOutcome};
use cbtc_domain::OccupancyState;
use uuid::Uuid;

async fn insert_test_block(pool: &sqlx::PgPool, name: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO signaling.blocks (id, name, length_m, max_speed_kph) VALUES ($1, $2, 200, 80)",
    )
    .bind(id)
    .bind(name)
    .execute(pool)
    .await
    .expect("block insert should succeed");
    id
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; run explicitly in CI with a live Postgres"]
async fn moving_train_clears_previous_block_and_occupies_new_one() {
    let pool = cbtc_testkit::test_pool("../../migrations").await;
    let repo = BlockRepository::new(&pool);

    let block_a = insert_test_block(&pool, "TEST-BLOCK-A").await;
    let block_b = insert_test_block(&pool, "TEST-BLOCK-B").await;
    let train_id = Uuid::new_v4();

    let outcome = repo.move_train_occupancy(train_id, block_a).await.unwrap();
    let MoveOutcome::Applied(changes) = outcome else {
        panic!("expected Applied, got {outcome:?}")
    };
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].0, block_a);
    assert_eq!(changes[0].2, OccupancyState::Occupied);

    let outcome = repo.move_train_occupancy(train_id, block_b).await.unwrap();
    let MoveOutcome::Applied(changes) = outcome else {
        panic!("expected Applied, got {outcome:?}")
    };
    assert_eq!(changes.len(), 2);

    let a_after = repo.find_by_id(block_a).await.unwrap().unwrap();
    let b_after = repo.find_by_id(block_b).await.unwrap().unwrap();
    assert_eq!(a_after.occupancy, OccupancyState::Clear);
    assert_eq!(a_after.occupied_by, None);
    assert_eq!(b_after.occupancy, OccupancyState::Occupied);
    assert_eq!(b_after.occupied_by, Some(train_id));
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; run explicitly in CI with a live Postgres"]
async fn re_reporting_same_block_produces_no_spurious_change() {
    let pool = cbtc_testkit::test_pool("../../migrations").await;
    let repo = BlockRepository::new(&pool);
    let block_a = insert_test_block(&pool, "TEST-BLOCK-IDEMPOTENT").await;
    let train_id = Uuid::new_v4();

    repo.move_train_occupancy(train_id, block_a).await.unwrap();
    let outcome = repo.move_train_occupancy(train_id, block_a).await.unwrap();
    let MoveOutcome::Applied(changes) = outcome else {
        panic!("expected Applied, got {outcome:?}")
    };

    assert!(
        changes.is_empty(),
        "re-reporting the same block for the same train must not produce a spurious occupancy transition"
    );
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; run explicitly in CI with a live Postgres"]
async fn conflicting_occupancy_report_is_rejected_not_overwritten() {
    let pool = cbtc_testkit::test_pool("../../migrations").await;
    let repo = BlockRepository::new(&pool);

    let block = insert_test_block(&pool, "TEST-BLOCK-CONFLICT").await;
    let train_a = Uuid::new_v4();
    let train_b = Uuid::new_v4();

    let outcome_a = repo.move_train_occupancy(train_a, block).await.unwrap();
    assert!(
        matches!(outcome_a, MoveOutcome::Applied(_)),
        "train A's initial claim should succeed"
    );

    let outcome_b = repo.move_train_occupancy(train_b, block).await.unwrap();
    match outcome_b {
        MoveOutcome::Conflict { incumbent_train_id } => {
            assert_eq!(
                incumbent_train_id, train_a,
                "the conflict must correctly identify the incumbent"
            );
        }
        MoveOutcome::Applied(_) => {
            panic!("train B's conflicting report must be rejected, not applied")
        }
    }

    let after = repo.find_by_id(block).await.unwrap().unwrap();
    assert_eq!(after.occupied_by, Some(train_a));
    assert_eq!(after.occupancy, OccupancyState::Occupied);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; run explicitly in CI with a live Postgres"]
async fn concurrent_conflicting_reports_for_the_same_block_never_both_apply() {
    let pool = cbtc_testkit::test_pool("../../migrations").await;
    let block = insert_test_block(&pool, "TEST-BLOCK-RACE").await;
    let train_a = Uuid::new_v4();
    let train_b = Uuid::new_v4();

    let repo = std::sync::Arc::new(BlockRepository::new(&pool));
    let repo_a = repo.clone();
    let repo_b = repo.clone();

    let (outcome_a, outcome_b) = tokio::join!(
        repo_a.move_train_occupancy(train_a, block),
        repo_b.move_train_occupancy(train_b, block),
    );

    let a_applied = matches!(outcome_a, Ok(MoveOutcome::Applied(_)));
    let b_applied = matches!(outcome_b, Ok(MoveOutcome::Applied(_)));

    assert!(
        a_applied ^ b_applied,
        "exactly one of two concurrent conflicting claims on a clear block must succeed, got a={a_applied} b={b_applied}"
    );
}
