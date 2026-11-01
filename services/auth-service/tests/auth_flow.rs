//! Integration tests for auth-service. Run with:
//!   TEST_DATABASE_URL=postgres://cbtc:cbtc@localhost:5432/cbtc_test cargo test -p auth-service

use argon2::password_hash::rand_core::OsRng;
use cbtc_auth::{hash_password, verify_password, JwtCodec, Role};

#[test]
fn password_hash_roundtrip() {
    let hash = hash_password("a-very-secure-password").expect("hash should succeed");
    assert!(verify_password("a-very-secure-password", &hash).expect("verify should not error"));
    assert!(!verify_password("wrong-password", &hash).expect("verify should not error"));
}

#[test]
fn password_hash_is_salted_uniquely() {
    let hash_a = hash_password("same-password-123456").unwrap();
    let hash_b = hash_password("same-password-123456").unwrap();
    assert_ne!(
        hash_a, hash_b,
        "two hashes of the same password must differ due to random salt"
    );
}

#[test]
fn jwt_roundtrip_and_role_claims() {
    let codec = JwtCodec::new("test-secret-key-not-for-production", 3600);
    let user_id = cbtc_domain::UserId::new();
    let token = codec
        .issue(user_id, vec![Role::Operator, Role::Viewer])
        .expect("issue should succeed");

    let claims = codec.validate(&token).expect("validate should succeed");
    assert_eq!(claims.sub, user_id);
    assert_eq!(claims.roles.len(), 2);
    assert!(claims.roles.contains(&Role::Operator));
}

#[test]
fn jwt_rejects_tampered_token() {
    let codec = JwtCodec::new("test-secret-key-not-for-production", 3600);
    let user_id = cbtc_domain::UserId::new();
    let mut token = codec.issue(user_id, vec![Role::Viewer]).unwrap();
    token.push_str("tampered");

    assert!(codec.validate(&token).is_err());
}

#[test]
fn jwt_from_different_secret_is_rejected() {
    let codec_a = JwtCodec::new("secret-a-not-for-production", 3600);
    let codec_b = JwtCodec::new("secret-b-not-for-production", 3600);
    let user_id = cbtc_domain::UserId::new();
    let token = codec_a.issue(user_id, vec![Role::Viewer]).unwrap();

    assert!(codec_b.validate(&token).is_err());
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; run explicitly in CI with a live Postgres"]
async fn register_then_login_then_fetch_profile() {
    let pool = cbtc_testkit::test_pool("../../migrations").await;

    let repo = auth_service::repository::UserRepository::new(&pool);
    let password_hash = hash_password("integration-test-password-1").unwrap();
    let user = repo
        .create_user("integration_tester", "tester@cbtc.local", &password_hash)
        .await
        .expect("user creation should succeed");

    repo.assign_role(user.id, Role::Operator)
        .await
        .expect("role assignment should succeed");

    let fetched = repo.find_by_username("integration_tester").await.unwrap();
    assert!(fetched.is_some());

    let roles = repo.roles_for(user.id).await.unwrap();
    assert_eq!(roles, vec![Role::Operator]);

    let _ = OsRng; // keep the argon2 rand_core import exercised across the module
}
