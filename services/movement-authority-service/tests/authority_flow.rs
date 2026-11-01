//! Run with: TEST_DATABASE_URL=postgres://cbtc:cbtc@localhost:5432/cbtc_test cargo test -p movement-authority-service

use cbtc_domain::AuthorityStatus;
use movement_authority_service::repository::AuthorityRepository;
use uuid::Uuid;

async fn insert_test_train(pool: &sqlx::PgPool, identifier: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO train.trains (id, identifier, operational_state, max_speed_kph, length_m) VALUES ($1, $2, 'staged', 80, 90)")
        .bind(id)
        .bind(identifier)
        .execute(pool)
        .await
        .expect("train insert should succeed");
    id
}

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
async fn granting_a_new_authority_supersedes_the_previous_one() {
    let pool = cbtc_testkit::test_pool("../../migrations").await;
    let repo = AuthorityRepository::new(&pool);

    let train_id = insert_test_train(&pool, "T-MA-TEST-001").await;
    let block_id = insert_test_block(&pool, "BLK-MA-TEST-001").await;

    let first = repo
        .grant(train_id, vec![block_id], 100.0, 60, 60)
        .await
        .unwrap();
    assert_eq!(first.status, AuthorityStatus::Granted);

    let second = repo
        .grant(train_id, vec![block_id], 200.0, 60, 60)
        .await
        .unwrap();
    assert_eq!(second.status, AuthorityStatus::Granted);

    let first_reloaded = repo.find_by_id(first.id).await.unwrap().unwrap();
    assert_eq!(
        first_reloaded.status,
        AuthorityStatus::Superseded,
        "granting a second authority must supersede the first"
    );

    let active = repo.find_active_for_train(train_id).await.unwrap();
    assert_eq!(active.unwrap().id, second.id);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; run explicitly in CI with a live Postgres"]
async fn concurrent_conflicting_authority_requests_never_both_succeed() {
    // Phase 6 / the core safety property of this service: two different
    // trains concurrently requesting authorities over an overlapping
    // block must never both be granted. This test launches both
    // requests at effectively the same instant (via tokio::join!, which
    // schedules both futures before either awaits its first .await
    // point in earnest â€” both HTTP-equivalent repository calls start
    // their transactions and race for the advisory lock genuinely
    // concurrently) and asserts exactly one succeeds.
    let pool = cbtc_testkit::test_pool("../../migrations").await;

    let train_a = insert_test_train(&pool, "T-RACE-A").await;
    let train_b = insert_test_train(&pool, "T-RACE-B").await;
    let shared_block = insert_test_block(&pool, "BLK-RACE-SHARED").await;

    let repo = std::sync::Arc::new(AuthorityRepository::new(&pool));
    let repo_a = repo.clone();
    let repo_b = repo.clone();

    let (result_a, result_b) = tokio::join!(
        repo_a.grant_if_no_conflict(train_a, vec![shared_block], 100.0, 60, 60),
        repo_b.grant_if_no_conflict(train_b, vec![shared_block], 100.0, 60, 60),
    );

    let a_granted = matches!(result_a, Ok(Ok(_)));
    let b_granted = matches!(result_b, Ok(Ok(_)));

    assert!(
        a_granted ^ b_granted,
        "exactly one of the two conflicting concurrent requests must succeed, got a_granted={a_granted} b_granted={b_granted}"
    );

    // Whichever lost must have lost with a structured conflict, not a
    // database error â€” a real error (e.g. a deadlock) would also make
    // `a_granted ^ b_granted` true by accident (one Err, one Ok), so
    // this assertion is what actually distinguishes "the safety check
    // worked" from "one request happened to crash".
    let loser = if a_granted { &result_b } else { &result_a };
    assert!(
        matches!(loser, Ok(Err(_))),
        "the losing request must fail via the structured conflict path (Ok(Err(blocks))), not an error: {loser:?}"
    );

    // And the winning authority must actually be the only live one over
    // the shared block once both requests have resolved.
    let live_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM train.movement_authorities WHERE status IN ('granted','active') AND $1 = ANY(block_sequence)",
    )
    .bind(shared_block)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        live_count.0, 1,
        "exactly one live authority must cover the contested block after the race resolves"
    );
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; run explicitly in CI with a live Postgres"]
async fn non_overlapping_concurrent_requests_both_succeed() {
    // Sanity check on the other side of the same property: the advisory
    // locking must not be so coarse that it serializes unrelated
    // requests into artificial conflicts. Two trains requesting
    // authorities over two entirely disjoint blocks must both succeed.
    let pool = cbtc_testkit::test_pool("../../migrations").await;

    let train_a = insert_test_train(&pool, "T-DISJOINT-A").await;
    let train_b = insert_test_train(&pool, "T-DISJOINT-B").await;
    let block_a = insert_test_block(&pool, "BLK-DISJOINT-A").await;
    let block_b = insert_test_block(&pool, "BLK-DISJOINT-B").await;

    let repo = std::sync::Arc::new(AuthorityRepository::new(&pool));
    let repo_a = repo.clone();
    let repo_b = repo.clone();

    let (result_a, result_b) = tokio::join!(
        repo_a.grant_if_no_conflict(train_a, vec![block_a], 100.0, 60, 60),
        repo_b.grant_if_no_conflict(train_b, vec![block_b], 100.0, 60, 60),
    );

    assert!(
        matches!(result_a, Ok(Ok(_))),
        "disjoint request A should succeed: {result_a:?}"
    );
    assert!(
        matches!(result_b, Ok(Ok(_))),
        "disjoint request B should succeed: {result_b:?}"
    );
}
