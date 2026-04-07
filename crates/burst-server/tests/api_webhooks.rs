mod common;

use axum::http::StatusCode;
use common::*;

/// Helper to set up an integrator user + channel for webhook tests.
async fn setup(pool: &sqlx::PgPool) -> (TestApp, String, String) {
    let app = TestApp::new(pool.clone());
    let user = seed_user_with_role(pool, "integrator", "integrator").await;
    let ch = seed_channel(pool, "webhook-test", user.id).await;
    let ext_id = "integrator@test.example".to_string();
    let ch_id = burst_core::id::format_channel_id(ch.id);
    (app, ext_id, ch_id)
}

#[sqlx::test(migrations = "../../migrations")]
async fn create_incoming_webhook(pool: sqlx::PgPool) {
    let (app, ext_id, ch_id) = setup(&pool).await;

    let (status, body) = app
        .post(
            &format!("/api/channels/{ch_id}/webhooks"),
            &ext_id,
            serde_json::json!({ "kind": "incoming", "name": "CI Bot" }),
        )
        .await;

    assert_eq!(status, StatusCode::CREATED, "body: {body}");
    assert_eq!(body["kind"], "incoming");
    assert_eq!(body["name"], "CI Bot");
    assert!(
        body["token"].is_string(),
        "token should be present on creation"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn create_outgoing_webhook_requires_url(pool: sqlx::PgPool) {
    let (app, ext_id, ch_id) = setup(&pool).await;

    let (status, _body) = app
        .post(
            &format!("/api/channels/{ch_id}/webhooks"),
            &ext_id,
            serde_json::json!({ "kind": "outgoing", "name": "Events" }),
        )
        .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../../migrations")]
async fn create_outgoing_webhook_with_url(pool: sqlx::PgPool) {
    let (app, ext_id, ch_id) = setup(&pool).await;

    let (status, body) = app
        .post(
            &format!("/api/channels/{ch_id}/webhooks"),
            &ext_id,
            serde_json::json!({
                "kind": "outgoing",
                "name": "Events",
                "url": "https://example.com/hook"
            }),
        )
        .await;

    assert_eq!(status, StatusCode::CREATED, "body: {body}");
    assert_eq!(body["kind"], "outgoing");
    assert_eq!(body["url"], "https://example.com/hook");
}

#[sqlx::test(migrations = "../../migrations")]
async fn list_webhooks(pool: sqlx::PgPool) {
    let (app, ext_id, ch_id) = setup(&pool).await;

    // Create two webhooks.
    app.post(
        &format!("/api/channels/{ch_id}/webhooks"),
        &ext_id,
        serde_json::json!({ "kind": "incoming", "name": "Hook 1" }),
    )
    .await;
    app.post(
        &format!("/api/channels/{ch_id}/webhooks"),
        &ext_id,
        serde_json::json!({ "kind": "incoming", "name": "Hook 2" }),
    )
    .await;

    let (status, body) = app
        .get(&format!("/api/channels/{ch_id}/webhooks"), &ext_id)
        .await;

    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["items"].as_array().unwrap().len(), 2);
}

#[sqlx::test(migrations = "../../migrations")]
async fn trigger_incoming_webhook(pool: sqlx::PgPool) {
    let user = seed_user_with_role(&pool, "integrator", "integrator").await;
    let ch = seed_channel(&pool, "trigger-test", user.id).await;
    let (wh, token) = seed_webhook(&pool, ch.id, "incoming", user.id).await;

    let app = TestApp::new(pool.clone());
    let wh_id = burst_core::id::format_webhook_id(wh.id);

    let (status, body) = app
        .post_with_bearer(
            &format!("/api/webhooks/{wh_id}/trigger"),
            &token,
            serde_json::json!({ "content": "Hello from CI!" }),
        )
        .await;

    assert_eq!(status, StatusCode::CREATED, "body: {body}");
    assert_eq!(body["content"], "Hello from CI!");
}

#[sqlx::test(migrations = "../../migrations")]
async fn trigger_with_bad_token(pool: sqlx::PgPool) {
    let user = seed_user_with_role(&pool, "integrator", "integrator").await;
    let ch = seed_channel(&pool, "bad-token-test", user.id).await;
    let (wh, _token) = seed_webhook(&pool, ch.id, "incoming", user.id).await;

    let app = TestApp::new(pool.clone());
    let wh_id = burst_core::id::format_webhook_id(wh.id);

    let (status, _body) = app
        .post_with_bearer(
            &format!("/api/webhooks/{wh_id}/trigger"),
            "wrong_token",
            serde_json::json!({ "content": "Should fail" }),
        )
        .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../../migrations")]
async fn trigger_outgoing_webhook_rejected(pool: sqlx::PgPool) {
    let user = seed_user_with_role(&pool, "integrator", "integrator").await;
    let ch = seed_channel(&pool, "outgoing-trigger-test", user.id).await;
    let (wh, token) = seed_webhook(&pool, ch.id, "outgoing", user.id).await;

    let app = TestApp::new(pool.clone());
    let wh_id = burst_core::id::format_webhook_id(wh.id);

    let (status, _body) = app
        .post_with_bearer(
            &format!("/api/webhooks/{wh_id}/trigger"),
            &token,
            serde_json::json!({ "content": "Should fail" }),
        )
        .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../../migrations")]
async fn trigger_inactive_webhook(pool: sqlx::PgPool) {
    let user = seed_user_with_role(&pool, "integrator", "integrator").await;
    let ch = seed_channel(&pool, "inactive-test", user.id).await;
    let (wh, token) = seed_webhook(&pool, ch.id, "incoming", user.id).await;

    // Deactivate the webhook.
    burst_server::db::webhooks::update(
        &pool,
        wh.id,
        &burst_server::db::webhooks::UpdateWebhook {
            name: None,
            url: None,
            is_active: Some(false),
        },
    )
    .await
    .unwrap();

    let app = TestApp::new(pool.clone());
    let wh_id = burst_core::id::format_webhook_id(wh.id);

    let (status, _body) = app
        .post_with_bearer(
            &format!("/api/webhooks/{wh_id}/trigger"),
            &token,
            serde_json::json!({ "content": "Should fail" }),
        )
        .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../migrations")]
async fn member_cannot_manage_webhooks(pool: sqlx::PgPool) {
    let member = seed_user(&pool, "member").await;
    let ch = seed_channel(&pool, "forbidden-test", member.id).await;
    let ch_id = burst_core::id::format_channel_id(ch.id);

    let app = TestApp::new(pool.clone());

    let (status, _body) = app
        .post(
            &format!("/api/channels/{ch_id}/webhooks"),
            "member@test.example",
            serde_json::json!({ "kind": "incoming", "name": "Nope" }),
        )
        .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../../migrations")]
async fn admin_can_manage_webhooks(pool: sqlx::PgPool) {
    let admin = seed_user_with_role(&pool, "admin", "admin").await;
    let ch = seed_channel(&pool, "admin-webhook-test", admin.id).await;
    let ch_id = burst_core::id::format_channel_id(ch.id);

    let app = TestApp::new(pool.clone());

    let (status, body) = app
        .post(
            &format!("/api/channels/{ch_id}/webhooks"),
            "admin@test.example",
            serde_json::json!({ "kind": "incoming", "name": "Admin Hook" }),
        )
        .await;

    assert_eq!(status, StatusCode::CREATED, "body: {body}");
}

#[sqlx::test(migrations = "../../migrations")]
async fn delete_webhook(pool: sqlx::PgPool) {
    let (app, ext_id, ch_id) = setup(&pool).await;

    let (_, created) = app
        .post(
            &format!("/api/channels/{ch_id}/webhooks"),
            &ext_id,
            serde_json::json!({ "kind": "incoming", "name": "To Delete" }),
        )
        .await;

    let wh_id = created["id"].as_str().unwrap();

    let (status, _body) = app
        .delete(&format!("/api/channels/{ch_id}/webhooks/{wh_id}"), &ext_id)
        .await;

    assert_eq!(status, StatusCode::NO_CONTENT);

    // Verify it's gone.
    let (status, _body) = app
        .get(&format!("/api/channels/{ch_id}/webhooks/{wh_id}"), &ext_id)
        .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../migrations")]
async fn update_webhook(pool: sqlx::PgPool) {
    let (app, ext_id, ch_id) = setup(&pool).await;

    let (_, created) = app
        .post(
            &format!("/api/channels/{ch_id}/webhooks"),
            &ext_id,
            serde_json::json!({ "kind": "incoming", "name": "Original" }),
        )
        .await;

    let wh_id = created["id"].as_str().unwrap();

    let (status, body) = app
        .patch(
            &format!("/api/channels/{ch_id}/webhooks/{wh_id}"),
            &ext_id,
            serde_json::json!({ "name": "Renamed", "isActive": false }),
        )
        .await;

    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["name"], "Renamed");
    assert_eq!(body["isActive"], false);
}

#[sqlx::test(migrations = "../../migrations")]
async fn list_all_webhooks_admin(pool: sqlx::PgPool) {
    let (app, ext_id, ch_id) = setup(&pool).await;

    app.post(
        &format!("/api/channels/{ch_id}/webhooks"),
        &ext_id,
        serde_json::json!({ "kind": "incoming", "name": "Global List" }),
    )
    .await;

    let (status, body) = app.get("/api/admin/webhooks", &ext_id).await;

    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert!(!body["items"].as_array().unwrap().is_empty());
}
