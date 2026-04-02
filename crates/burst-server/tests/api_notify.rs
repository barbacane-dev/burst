mod common;

use axum::http::StatusCode;

fn auth(username: &str) -> String {
    format!("{username}@test.example")
}

// ── Notification preference ──────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn update_notify_to_mentions(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await;
    let ch = common::seed_channel(&pool, "general", alice.id).await;
    let ch_id = format!("ch_{}", ch.id);

    let (status, _) = app
        .patch(
            &format!("/api/channels/{ch_id}/members/me/notify"),
            &auth("alice"),
            serde_json::json!({ "notify": "mentions" }),
        )
        .await;

    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[sqlx::test(migrations = "../../migrations")]
async fn update_notify_to_all(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await;
    let ch = common::seed_channel(&pool, "general", alice.id).await;
    let ch_id = format!("ch_{}", ch.id);

    let (status, _) = app
        .patch(
            &format!("/api/channels/{ch_id}/members/me/notify"),
            &auth("alice"),
            serde_json::json!({ "notify": "all" }),
        )
        .await;

    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[sqlx::test(migrations = "../../migrations")]
async fn update_notify_to_nothing(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await;
    let ch = common::seed_channel(&pool, "general", alice.id).await;
    let ch_id = format!("ch_{}", ch.id);

    let (status, _) = app
        .patch(
            &format!("/api/channels/{ch_id}/members/me/notify"),
            &auth("alice"),
            serde_json::json!({ "notify": "nothing" }),
        )
        .await;

    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[sqlx::test(migrations = "../../migrations")]
async fn update_notify_invalid_value_is_bad_request(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await;
    let ch = common::seed_channel(&pool, "general", alice.id).await;
    let ch_id = format!("ch_{}", ch.id);

    let (status, _) = app
        .patch(
            &format!("/api/channels/{ch_id}/members/me/notify"),
            &auth("alice"),
            serde_json::json!({ "notify": "invalid" }),
        )
        .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../../migrations")]
async fn update_notify_non_member_is_not_found(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await;
    let ch = common::seed_channel(&pool, "general", alice.id).await;
    common::seed_user(&pool, "eve").await;

    let ch_id = format!("ch_{}", ch.id);

    let (status, _) = app
        .patch(
            &format!("/api/channels/{ch_id}/members/me/notify"),
            &auth("eve"),
            serde_json::json!({ "notify": "all" }),
        )
        .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}
