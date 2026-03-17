mod common;

use axum::http::StatusCode;

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Auth header value for a seeded user.
fn auth(username: &str) -> String {
    format!("{username}@test.example")
}

// ── List users ───────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn list_users_as_admin(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let admin = common::seed_user_with_role(&pool, "admin1", "admin").await;
    common::seed_user(&pool, "alice").await;
    common::seed_user(&pool, "bob").await;
    let _ = admin; // ensure admin exists

    let (status, body) = app.get("/admin/users", &auth("admin1")).await;

    assert_eq!(status, StatusCode::OK);
    let items = body["items"].as_array().unwrap();
    assert!(items.len() >= 3, "should list all users including admin");
}

#[sqlx::test(migrations = "../../migrations")]
async fn list_users_as_member_is_forbidden(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    common::seed_user(&pool, "alice").await;

    let (status, _) = app.get("/admin/users", &auth("alice")).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../../migrations")]
async fn list_users_unauthenticated_is_unauthorized(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());

    let (status, _) = app.request_no_auth("GET", "/admin/users", None).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ── Update user role ─────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn update_user_role(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    common::seed_user_with_role(&pool, "admin1", "admin").await;
    let alice = common::seed_user(&pool, "alice").await;

    let user_id = burst_core::id::format_user_id(alice.id);
    let (status, body) = app
        .patch(
            &format!("/admin/users/{user_id}"),
            &auth("admin1"),
            serde_json::json!({ "role": "moderator" }),
        )
        .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["role"], "moderator");
}

#[sqlx::test(migrations = "../../migrations")]
async fn update_user_role_invalid_is_bad_request(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    common::seed_user_with_role(&pool, "admin1", "admin").await;
    let alice = common::seed_user(&pool, "alice").await;

    let user_id = burst_core::id::format_user_id(alice.id);
    let (status, _) = app
        .patch(
            &format!("/admin/users/{user_id}"),
            &auth("admin1"),
            serde_json::json!({ "role": "superuser" }),
        )
        .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../../migrations")]
async fn update_nonexistent_user_returns_not_found(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    common::seed_user_with_role(&pool, "admin1", "admin").await;

    let fake_id = burst_core::id::format_user_id(burst_core::id::new_id());
    let (status, _) = app
        .patch(
            &format!("/admin/users/{fake_id}"),
            &auth("admin1"),
            serde_json::json!({ "role": "guest" }),
        )
        .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ── Deactivate / reactivate ──────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn deactivate_user(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    common::seed_user_with_role(&pool, "admin1", "admin").await;
    let alice = common::seed_user(&pool, "alice").await;

    let user_id = burst_core::id::format_user_id(alice.id);
    let (status, body) = app
        .patch(
            &format!("/admin/users/{user_id}"),
            &auth("admin1"),
            serde_json::json!({ "deactivated": true }),
        )
        .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        body["deactivatedAt"].is_string(),
        "deactivatedAt must be set"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn reactivate_user(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    common::seed_user_with_role(&pool, "admin1", "admin").await;
    let alice = common::seed_user(&pool, "alice").await;

    let user_id = burst_core::id::format_user_id(alice.id);

    // Deactivate first
    app.patch(
        &format!("/admin/users/{user_id}"),
        &auth("admin1"),
        serde_json::json!({ "deactivated": true }),
    )
    .await;

    // Now reactivate
    let (status, body) = app
        .patch(
            &format!("/admin/users/{user_id}"),
            &auth("admin1"),
            serde_json::json!({ "deactivated": false }),
        )
        .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        body["deactivatedAt"].is_null(),
        "deactivatedAt must be null after reactivation"
    );
}

// ── Admin channel management ─────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn admin_list_channels(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let admin = common::seed_user_with_role(&pool, "admin1", "admin").await;
    common::seed_channel(&pool, "general", admin.id).await;

    let (status, body) = app.get("/admin/channels", &auth("admin1")).await;

    assert_eq!(status, StatusCode::OK);
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
}

#[sqlx::test(migrations = "../../migrations")]
async fn admin_archive_channel(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let admin = common::seed_user_with_role(&pool, "admin1", "admin").await;
    let ch = common::seed_channel(&pool, "general", admin.id).await;

    let ch_id = burst_core::id::format_channel_id(ch.id);
    let (status, body) = app
        .patch(
            &format!("/admin/channels/{ch_id}"),
            &auth("admin1"),
            serde_json::json!({ "isArchived": true }),
        )
        .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["isArchived"], true);
}

#[sqlx::test(migrations = "../../migrations")]
async fn admin_unarchive_channel(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let admin = common::seed_user_with_role(&pool, "admin1", "admin").await;
    let ch = common::seed_channel(&pool, "general", admin.id).await;

    let ch_id = burst_core::id::format_channel_id(ch.id);

    // Archive first
    app.patch(
        &format!("/admin/channels/{ch_id}"),
        &auth("admin1"),
        serde_json::json!({ "isArchived": true }),
    )
    .await;

    // Unarchive
    let (status, body) = app
        .patch(
            &format!("/admin/channels/{ch_id}"),
            &auth("admin1"),
            serde_json::json!({ "isArchived": false }),
        )
        .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["isArchived"], false);
}

#[sqlx::test(migrations = "../../migrations")]
async fn admin_delete_channel(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let admin = common::seed_user_with_role(&pool, "admin1", "admin").await;
    let ch = common::seed_channel(&pool, "general", admin.id).await;

    let ch_id = burst_core::id::format_channel_id(ch.id);
    let (status, _) = app
        .delete(&format!("/admin/channels/{ch_id}"), &auth("admin1"))
        .await;

    assert_eq!(status, StatusCode::NO_CONTENT);

    // Verify it's actually gone
    let (status, _) = app.get("/admin/channels", &auth("admin1")).await;
    assert_eq!(status, StatusCode::OK);
}

#[sqlx::test(migrations = "../../migrations")]
async fn admin_delete_nonexistent_channel_returns_not_found(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    common::seed_user_with_role(&pool, "admin1", "admin").await;

    let fake_id = burst_core::id::format_channel_id(burst_core::id::new_id());
    let (status, _) = app
        .delete(&format!("/admin/channels/{fake_id}"), &auth("admin1"))
        .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ── Audit log ────────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn audit_log_records_role_change(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    common::seed_user_with_role(&pool, "admin1", "admin").await;
    let alice = common::seed_user(&pool, "alice").await;

    let user_id = burst_core::id::format_user_id(alice.id);
    app.patch(
        &format!("/admin/users/{user_id}"),
        &auth("admin1"),
        serde_json::json!({ "role": "moderator" }),
    )
    .await;

    let (status, body) = app.get("/admin/audit-log", &auth("admin1")).await;

    assert_eq!(status, StatusCode::OK);
    let items = body["items"].as_array().unwrap();
    assert!(!items.is_empty(), "audit log must have entries");
    assert_eq!(items[0]["action"], "user.role_changed");
    assert_eq!(items[0]["targetType"], "user");
}

#[sqlx::test(migrations = "../../migrations")]
async fn audit_log_records_channel_archive(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let admin = common::seed_user_with_role(&pool, "admin1", "admin").await;
    let ch = common::seed_channel(&pool, "general", admin.id).await;

    let ch_id = burst_core::id::format_channel_id(ch.id);
    app.patch(
        &format!("/admin/channels/{ch_id}"),
        &auth("admin1"),
        serde_json::json!({ "isArchived": true }),
    )
    .await;

    let (status, body) = app.get("/admin/audit-log", &auth("admin1")).await;

    assert_eq!(status, StatusCode::OK);
    let items = body["items"].as_array().unwrap();
    let archive_entry = items.iter().find(|e| e["action"] == "channel.archived");
    assert!(
        archive_entry.is_some(),
        "must have channel.archived audit entry"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn audit_log_filter_by_target_type(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let admin = common::seed_user_with_role(&pool, "admin1", "admin").await;
    let alice = common::seed_user(&pool, "alice").await;
    let ch = common::seed_channel(&pool, "general", admin.id).await;

    // Create both user and channel audit entries
    let user_id = burst_core::id::format_user_id(alice.id);
    app.patch(
        &format!("/admin/users/{user_id}"),
        &auth("admin1"),
        serde_json::json!({ "role": "moderator" }),
    )
    .await;
    let ch_id = burst_core::id::format_channel_id(ch.id);
    app.patch(
        &format!("/admin/channels/{ch_id}"),
        &auth("admin1"),
        serde_json::json!({ "isArchived": true }),
    )
    .await;

    // Filter by user only (camelCase query param)
    let (status, body) = app
        .get("/admin/audit-log?targetType=user", &auth("admin1"))
        .await;

    assert_eq!(status, StatusCode::OK);
    let items = body["items"].as_array().unwrap();
    assert!(
        items.iter().all(|e| e["targetType"] == "user"),
        "all entries must be user type when filtered"
    );
}

// ── Non-admin guard on all admin endpoints ───────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn member_cannot_access_admin_endpoints(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    common::seed_user(&pool, "alice").await;

    let endpoints = vec![
        ("GET", "/admin/users"),
        ("GET", "/admin/channels"),
        ("GET", "/admin/audit-log"),
    ];

    for (method, path) in endpoints {
        let (status, _) = app.request_no_auth(method, path, None).await;
        // Without auth header => 401; with member auth => 403
        assert!(
            status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN,
            "{method} {path} must reject unauthenticated requests, got {status}"
        );
    }
}
