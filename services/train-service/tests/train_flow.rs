//! Run with: TEST_DATABASE_URL=postgres://cbtc:cbtc@localhost:5432/cbtc_test cargo test -p train-service

use cbtc_domain::{Train, TrainOperationalState};
use chrono::Utc;
use train_service::repository::TrainRepository;

#[test]
fn heartbeat_timeout_detection_is_symmetric_with_domain_constant() {
    // train-service's SQL staleness query and cbtc_domain::Train's own
    // has_lost_communication() must agree on the same timeout constant,
    // or a train could show as "communicating" in one service and
    // "stale" in another. This test pins that invariant.
    let mut train = cbtc_testkit::fixture_train();
    train.last_heartbeat =
        Utc::now() - chrono::Duration::seconds(Train::HEARTBEAT_TIMEOUT_SECS + 1);
    assert!(train.has_lost_communication(Utc::now()));

    train.last_heartbeat =
        Utc::now() - chrono::Duration::seconds(Train::HEARTBEAT_TIMEOUT_SECS - 1);
    assert!(!train.has_lost_communication(Utc::now()));
}

#[test]
fn blocks_new_authority_covers_all_unsafe_states() {
    assert!(TrainOperationalState::EmergencyBraking.blocks_new_authority());
    assert!(TrainOperationalState::Degraded.blocks_new_authority());
    assert!(TrainOperationalState::CommunicationLost.blocks_new_authority());
    assert!(TrainOperationalState::OutOfService.blocks_new_authority());
    assert!(!TrainOperationalState::InMotion.blocks_new_authority());
    assert!(!TrainOperationalState::Stopped.blocks_new_authority());
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; run explicitly in CI with a live Postgres"]
async fn create_train_then_record_position_report() {
    let pool = cbtc_testkit::test_pool("../../migrations").await;
    let repo = TrainRepository::new(&pool);

    let created = repo
        .create("T-TEST-001", 80, 90.0, None)
        .await
        .expect("train creation should succeed");
    assert_eq!(created.operational_state, TrainOperationalState::Staged);

    // Insert a block directly to satisfy the FK on position_reports;
    // block-occupancy-service owns block creation in the real system,
    // this test only needs a valid FK target.
    let block_id = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO signaling.blocks (id, name, length_m, max_speed_kph) VALUES ($1, $2, 100, 80)",
    )
    .bind(block_id)
    .bind(format!("BLK-TEST-{}", &block_id.simple().to_string()[..6]))
    .execute(&pool)
    .await
    .expect("block insert should succeed");

    repo.record_position_report(created.id, block_id, 15.0, 42.0, 0.5, Utc::now())
        .await
        .expect("position report should succeed");

    let updated = repo.find_by_id(created.id).await.unwrap().unwrap();
    assert_eq!(updated.current_block, Some(block_id));
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; run explicitly in CI with a live Postgres"]
async fn out_of_order_position_report_is_rejected_without_corrupting_current_state() {
    let pool = cbtc_testkit::test_pool("../../migrations").await;
    let repo = TrainRepository::new(&pool);

    let created = repo.create("T-TEST-OOO", 80, 90.0, None).await.unwrap();

    let block_first = uuid::Uuid::new_v4();
    let block_second = uuid::Uuid::new_v4();
    for (id, name) in [(block_first, "BLK-OOO-1"), (block_second, "BLK-OOO-2")] {
        sqlx::query("INSERT INTO signaling.blocks (id, name, length_m, max_speed_kph) VALUES ($1, $2, 100, 80)")
            .bind(id)
            .bind(name)
            .execute(&pool)
            .await
            .unwrap();
    }

    let now = Utc::now();
    let later = now + chrono::Duration::seconds(5);
    let earlier = now - chrono::Duration::seconds(5);

    // Apply the "later" report first (simulating network reordering —
    // it happened second in train-time but arrived first).
    let applied_later = repo
        .record_position_report(created.id, block_second, 0.0, 40.0, 0.0, later)
        .await
        .unwrap();
    assert!(
        applied_later,
        "the first-arriving report should always be applied"
    );

    // Now the logically-earlier report arrives, delayed.
    let applied_earlier = repo
        .record_position_report(created.id, block_first, 0.0, 40.0, 0.0, earlier)
        .await
        .unwrap();
    assert!(
        !applied_earlier,
        "an out-of-order report arriving after a newer one must be rejected"
    );

    // The train's current_block must still reflect the newer report,
    // not have been clobbered by the stale one.
    let final_state = repo.find_by_id(created.id).await.unwrap().unwrap();
    assert_eq!(
        final_state.current_block,
        Some(block_second),
        "current_block must reflect the logically newest report, not the most recently arrived one"
    );
}
