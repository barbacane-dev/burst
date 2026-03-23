mod common;

use axum::http::StatusCode;

#[sqlx::test(migrations = "../../migrations")]
async fn update_display_name(pool: sqlx::PgPool) {
    let user = common::seed_user(&pool, "alice").await;
    let app = common::TestApp::new(pool);
    let eid = user.external_id.as_deref().expect("external_id");

    let (status, body) = app
        .patch(
            "/users/me",
            eid,
            serde_json::json!({ "displayName": "Alice Updated" }),
        )
        .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["displayName"], "Alice Updated");
    assert_eq!(body["username"], "alice");
}

#[sqlx::test(migrations = "../../migrations")]
async fn update_email(pool: sqlx::PgPool) {
    let user = common::seed_user(&pool, "bob").await;
    let app = common::TestApp::new(pool);
    let eid = user.external_id.as_deref().expect("external_id");

    let (status, body) = app
        .patch(
            "/users/me",
            eid,
            serde_json::json!({ "email": "bob@newdomain.com" }),
        )
        .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["email"], "bob@newdomain.com");
}

#[sqlx::test(migrations = "../../migrations")]
async fn update_status_text(pool: sqlx::PgPool) {
    let user = common::seed_user(&pool, "carol").await;
    let app = common::TestApp::new(pool);
    let eid = user.external_id.as_deref().expect("external_id");

    let (status, body) = app
        .patch(
            "/users/me",
            eid,
            serde_json::json!({ "statusText": "On vacation" }),
        )
        .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["statusText"], "On vacation");
}

#[sqlx::test(migrations = "../../migrations")]
async fn update_multiple_fields(pool: sqlx::PgPool) {
    let user = common::seed_user(&pool, "dave").await;
    let app = common::TestApp::new(pool);
    let eid = user.external_id.as_deref().expect("external_id");

    let (status, body) = app
        .patch(
            "/users/me",
            eid,
            serde_json::json!({
                "displayName": "Dave New",
                "email": "dave@example.com",
                "statusText": "Busy"
            }),
        )
        .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["displayName"], "Dave New");
    assert_eq!(body["email"], "dave@example.com");
    assert_eq!(body["statusText"], "Busy");
}

#[sqlx::test(migrations = "../../migrations")]
async fn update_profile_unauthenticated(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool);

    let (status, _body) = app
        .request_no_auth(
            "PATCH",
            "/users/me",
            Some(serde_json::json!({ "displayName": "Hacker" })),
        )
        .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
