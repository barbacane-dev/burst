mod common;

use axum::http::StatusCode;
use common::*;

#[sqlx::test(migrations = "../../migrations")]
async fn integrator_creates_bot(pool: sqlx::PgPool) {
    let _integrator = seed_user_with_role(&pool, "integrator", "integrator").await;
    let app = TestApp::new(pool.clone());

    let (status, body) = app
        .post(
            "/api/admin/bots",
            "integrator@test.example",
            serde_json::json!({ "username": "ci-bot", "displayName": "CI Bot" }),
        )
        .await;

    assert_eq!(status, StatusCode::CREATED, "body: {body}");
    assert_eq!(body["username"], "ci-bot");
    assert_eq!(body["displayName"], "CI Bot");
    assert_eq!(body["isBot"], true);
}

#[sqlx::test(migrations = "../../migrations")]
async fn admin_creates_bot(pool: sqlx::PgPool) {
    let _admin = seed_user_with_role(&pool, "admin", "admin").await;
    let app = TestApp::new(pool.clone());

    let (status, body) = app
        .post(
            "/api/admin/bots",
            "admin@test.example",
            serde_json::json!({ "username": "deploy-bot", "displayName": "Deploy Bot" }),
        )
        .await;

    assert_eq!(status, StatusCode::CREATED, "body: {body}");
    assert_eq!(body["isBot"], true);
}

#[sqlx::test(migrations = "../../migrations")]
async fn member_cannot_create_bot(pool: sqlx::PgPool) {
    let _member = seed_user(&pool, "member").await;
    let app = TestApp::new(pool.clone());

    let (status, _body) = app
        .post(
            "/api/admin/bots",
            "member@test.example",
            serde_json::json!({ "username": "nope-bot", "displayName": "Nope" }),
        )
        .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../../migrations")]
async fn list_bots(pool: sqlx::PgPool) {
    let _integrator = seed_user_with_role(&pool, "integrator", "integrator").await;
    let app = TestApp::new(pool.clone());

    app.post(
        "/api/admin/bots",
        "integrator@test.example",
        serde_json::json!({ "username": "bot-a", "displayName": "Bot A" }),
    )
    .await;
    app.post(
        "/api/admin/bots",
        "integrator@test.example",
        serde_json::json!({ "username": "bot-b", "displayName": "Bot B" }),
    )
    .await;

    let (status, body) = app.get("/api/admin/bots", "integrator@test.example").await;

    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["items"].as_array().unwrap().len(), 2);
}

#[sqlx::test(migrations = "../../migrations")]
async fn update_bot_display_name(pool: sqlx::PgPool) {
    let _integrator = seed_user_with_role(&pool, "integrator", "integrator").await;
    let app = TestApp::new(pool.clone());

    let (_, created) = app
        .post(
            "/api/admin/bots",
            "integrator@test.example",
            serde_json::json!({ "username": "rename-bot", "displayName": "Old Name" }),
        )
        .await;

    let bot_id = created["id"].as_str().unwrap();

    let (status, body) = app
        .patch(
            &format!("/api/admin/bots/{bot_id}"),
            "integrator@test.example",
            serde_json::json!({ "displayName": "New Name" }),
        )
        .await;

    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["displayName"], "New Name");
}

#[sqlx::test(migrations = "../../migrations")]
async fn deactivate_bot(pool: sqlx::PgPool) {
    let _integrator = seed_user_with_role(&pool, "integrator", "integrator").await;
    let app = TestApp::new(pool.clone());

    let (_, created) = app
        .post(
            "/api/admin/bots",
            "integrator@test.example",
            serde_json::json!({ "username": "deactivate-bot", "displayName": "Doomed" }),
        )
        .await;

    let bot_id = created["id"].as_str().unwrap();

    let (status, _body) = app
        .delete(
            &format!("/api/admin/bots/{bot_id}"),
            "integrator@test.example",
        )
        .await;

    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[sqlx::test(migrations = "../../migrations")]
async fn duplicate_username_returns_conflict(pool: sqlx::PgPool) {
    let _integrator = seed_user_with_role(&pool, "integrator", "integrator").await;
    let app = TestApp::new(pool.clone());

    app.post(
        "/api/admin/bots",
        "integrator@test.example",
        serde_json::json!({ "username": "unique-bot", "displayName": "First" }),
    )
    .await;

    let (status, _body) = app
        .post(
            "/api/admin/bots",
            "integrator@test.example",
            serde_json::json!({ "username": "unique-bot", "displayName": "Second" }),
        )
        .await;

    assert_eq!(status, StatusCode::CONFLICT);
}
