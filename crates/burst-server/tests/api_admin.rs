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

    let (status, body) = app.get("/api/admin/users", &auth("admin1")).await;

    assert_eq!(status, StatusCode::OK);
    let items = body["items"].as_array().unwrap();
    assert!(items.len() >= 3, "should list all users including admin");
}

#[sqlx::test(migrations = "../../migrations")]
async fn list_users_as_member_is_forbidden(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    common::seed_user(&pool, "alice").await;

    let (status, _) = app.get("/api/admin/users", &auth("alice")).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../../migrations")]
async fn list_users_unauthenticated_is_unauthorized(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());

    let (status, _) = app.request_no_auth("GET", "/api/admin/users", None).await;

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
            &format!("/api/admin/users/{user_id}"),
            &auth("admin1"),
            serde_json::json!({ "role": "moderator" }),
        )
        .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["role"], "moderator");
}

/// A role an admin sets by hand must survive the user's next request. The auth
/// plugins omit the groups header for an identity that has none, and reading
/// that absence as `member` silently undid the change on the very next call.
#[sqlx::test(migrations = "../../migrations")]
async fn a_role_set_by_an_admin_survives_the_next_request(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    common::seed_user_with_role(&pool, "admin1", "admin").await;
    let alice = common::seed_user(&pool, "alice").await;
    let user_id = burst_core::id::format_user_id(alice.id);

    let (status, body) = app
        .patch(
            &format!("/api/admin/users/{user_id}"),
            &auth("admin1"),
            serde_json::json!({ "role": "moderator" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["role"], "moderator");

    // Alice calls again. The gateway sends her claims, as it does on every
    // request, but her identity has no groups so it sends no groups header.
    // The stored role must stand.
    let (status, _) = app
        .request_with_headers(
            "GET",
            "/api/users/me",
            &auth("alice"),
            &[("x-auth-claims", r#"{"sub":"alice"}"#)],
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let stored = burst_server::db::users::find_by_external_id(&pool, &auth("alice"))
        .await
        .expect("query")
        .expect("alice exists");
    assert_eq!(
        stored.role, "moderator",
        "the role an admin set was overwritten by the groups re-sync"
    );
}

/// Where the identity provider does say something, it still wins: that is the
/// re-sync the role mapping exists for.
#[sqlx::test(migrations = "../../migrations")]
async fn groups_still_drive_the_role_when_the_gateway_sends_them(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await;

    let (status, _) = app
        .request_with_headers(
            "GET",
            "/api/users/me",
            &auth("alice"),
            &[
                ("x-auth-consumer-groups", "admin"),
                // The re-sync runs inside the claims block, so the gateway's
                // claims header has to be present as it is in a real request.
                ("x-auth-claims", r#"{"sub":"alice"}"#),
            ],
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let stored = burst_server::db::users::find_by_external_id(&pool, &auth("alice"))
        .await
        .expect("query")
        .expect("alice exists");
    assert_eq!(stored.role, "admin", "a groups header still promotes");
    let _ = alice;
}

#[sqlx::test(migrations = "../../migrations")]
async fn update_user_role_invalid_is_bad_request(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    common::seed_user_with_role(&pool, "admin1", "admin").await;
    let alice = common::seed_user(&pool, "alice").await;

    let user_id = burst_core::id::format_user_id(alice.id);
    let (status, _) = app
        .patch(
            &format!("/api/admin/users/{user_id}"),
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
            &format!("/api/admin/users/{fake_id}"),
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
            &format!("/api/admin/users/{user_id}"),
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
        &format!("/api/admin/users/{user_id}"),
        &auth("admin1"),
        serde_json::json!({ "deactivated": true }),
    )
    .await;

    // Now reactivate
    let (status, body) = app
        .patch(
            &format!("/api/admin/users/{user_id}"),
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

    let (status, body) = app.get("/api/admin/channels", &auth("admin1")).await;

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
            &format!("/api/admin/channels/{ch_id}"),
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
        &format!("/api/admin/channels/{ch_id}"),
        &auth("admin1"),
        serde_json::json!({ "isArchived": true }),
    )
    .await;

    // Unarchive
    let (status, body) = app
        .patch(
            &format!("/api/admin/channels/{ch_id}"),
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
        .delete(&format!("/api/admin/channels/{ch_id}"), &auth("admin1"))
        .await;

    assert_eq!(status, StatusCode::NO_CONTENT);

    // Verify it's actually gone
    let (status, _) = app.get("/api/admin/channels", &auth("admin1")).await;
    assert_eq!(status, StatusCode::OK);
}

#[sqlx::test(migrations = "../../migrations")]
async fn admin_delete_nonexistent_channel_returns_not_found(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    common::seed_user_with_role(&pool, "admin1", "admin").await;

    let fake_id = burst_core::id::format_channel_id(burst_core::id::new_id());
    let (status, _) = app
        .delete(&format!("/api/admin/channels/{fake_id}"), &auth("admin1"))
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
        &format!("/api/admin/users/{user_id}"),
        &auth("admin1"),
        serde_json::json!({ "role": "moderator" }),
    )
    .await;

    let (status, body) = app.get("/api/admin/audit-log", &auth("admin1")).await;

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
        &format!("/api/admin/channels/{ch_id}"),
        &auth("admin1"),
        serde_json::json!({ "isArchived": true }),
    )
    .await;

    let (status, body) = app.get("/api/admin/audit-log", &auth("admin1")).await;

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
        &format!("/api/admin/users/{user_id}"),
        &auth("admin1"),
        serde_json::json!({ "role": "moderator" }),
    )
    .await;
    let ch_id = burst_core::id::format_channel_id(ch.id);
    app.patch(
        &format!("/api/admin/channels/{ch_id}"),
        &auth("admin1"),
        serde_json::json!({ "isArchived": true }),
    )
    .await;

    // Filter by user only (camelCase query param)
    let (status, body) = app
        .get("/api/admin/audit-log?targetType=user", &auth("admin1"))
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
        ("GET", "/api/admin/users"),
        ("GET", "/api/admin/channels"),
        ("GET", "/api/admin/audit-log"),
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
