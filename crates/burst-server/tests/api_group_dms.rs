mod common;

use axum::http::StatusCode;

#[sqlx::test(migrations = "../../migrations")]
async fn create_group_dm(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool);
    let alice = common::seed_user(&app.state.db, "alice").await;
    let bob = common::seed_user(&app.state.db, "bob").await;
    let charlie = common::seed_user(&app.state.db, "charlie").await;

    let _alice_id = burst_core::id::format_user_id(alice.id);
    let bob_id = burst_core::id::format_user_id(bob.id);
    let charlie_id = burst_core::id::format_user_id(charlie.id);

    let (status, body) = app
        .post(
            "/dms",
            "alice@test.example",
            serde_json::json!({ "userIds": [bob_id, charlie_id] }),
        )
        .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["kind"], "group_dm");
}

#[sqlx::test(migrations = "../../migrations")]
async fn group_dm_is_idempotent(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool);
    let _alice = common::seed_user(&app.state.db, "alice").await;
    let bob = common::seed_user(&app.state.db, "bob").await;
    let charlie = common::seed_user(&app.state.db, "charlie").await;

    let bob_id = burst_core::id::format_user_id(bob.id);
    let charlie_id = burst_core::id::format_user_id(charlie.id);

    let (_, body1) = app
        .post(
            "/dms",
            "alice@test.example",
            serde_json::json!({ "userIds": [bob_id.clone(), charlie_id.clone()] }),
        )
        .await;

    let (_, body2) = app
        .post(
            "/dms",
            "alice@test.example",
            serde_json::json!({ "userIds": [bob_id, charlie_id] }),
        )
        .await;

    assert_eq!(body1["id"], body2["id"], "should return same channel");
}

#[sqlx::test(migrations = "../../migrations")]
async fn single_user_id_array_creates_regular_dm(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool);
    let _alice = common::seed_user(&app.state.db, "alice").await;
    let bob = common::seed_user(&app.state.db, "bob").await;

    let bob_id = burst_core::id::format_user_id(bob.id);

    let (status, body) = app
        .post(
            "/dms",
            "alice@test.example",
            serde_json::json!({ "userIds": [bob_id] }),
        )
        .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["kind"], "dm");
}

#[sqlx::test(migrations = "../../migrations")]
async fn backward_compat_user_id_field(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool);
    let _alice = common::seed_user(&app.state.db, "alice").await;
    let bob = common::seed_user(&app.state.db, "bob").await;

    let bob_id = burst_core::id::format_user_id(bob.id);

    // Old format: { userId: "..." }
    let (status, body) = app
        .post(
            "/dms",
            "alice@test.example",
            serde_json::json!({ "userId": bob_id }),
        )
        .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["kind"], "dm");
}

#[sqlx::test(migrations = "../../migrations")]
async fn send_message_in_group_dm(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool);
    let _alice = common::seed_user(&app.state.db, "alice").await;
    let bob = common::seed_user(&app.state.db, "bob").await;
    let charlie = common::seed_user(&app.state.db, "charlie").await;

    let bob_id = burst_core::id::format_user_id(bob.id);
    let charlie_id = burst_core::id::format_user_id(charlie.id);

    let (_, ch) = app
        .post(
            "/dms",
            "alice@test.example",
            serde_json::json!({ "userIds": [bob_id, charlie_id] }),
        )
        .await;
    let channel_id = ch["id"].as_str().unwrap();

    let (status, msg) = app
        .post(
            &format!("/channels/{channel_id}/messages"),
            "alice@test.example",
            serde_json::json!({ "content": "hello group!" }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(msg["content"], "hello group!");
}
