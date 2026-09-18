mod common;

use axum::http::StatusCode;
use tower::ServiceExt;

#[sqlx::test(migrations = "../../migrations")]
async fn list_emojis_empty(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool);
    let _ = common::seed_user(&app.state.db, "alice").await;

    let (status, body) = app.get("/api/emojis", "alice@test.example").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["items"].as_array().unwrap().len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn admin_create_and_list_emoji(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool);
    let _ = common::seed_user_with_role(&app.state.db, "admin", "admin").await;

    // Create emoji via multipart
    let boundary = "----boundary";
    let body = format!(
        "--{boundary}\r\n\
         Content-Disposition: form-data; name=\"shortcode\"\r\n\r\n\
         partyparrot\r\n\
         --{boundary}\r\n\
         Content-Disposition: form-data; name=\"image\"; filename=\"parrot.png\"\r\n\
         Content-Type: image/png\r\n\r\n\
         fake-png-data\r\n\
         --{boundary}--\r\n"
    );

    let req = common::with_peer(axum::http::Request::builder(), "127.0.0.1:54321")
        .method("POST")
        .uri("/api/admin/emojis")
        .header("x-auth-consumer", "admin@test.example")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(axum::body::Body::from(body))
        .unwrap();

    let response = app.router.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let emoji: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(emoji["shortcode"], "partyparrot");

    // List emojis (public)
    let (status, body) = app.get("/api/emojis", "admin@test.example").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["items"].as_array().unwrap().len(), 1);
    assert_eq!(body["items"][0]["shortcode"], "partyparrot");
}

#[sqlx::test(migrations = "../../migrations")]
async fn non_admin_cannot_create_emoji(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool);
    let _ = common::seed_user(&app.state.db, "bob").await;

    let boundary = "----boundary";
    let body = format!(
        "--{boundary}\r\n\
         Content-Disposition: form-data; name=\"shortcode\"\r\n\r\n\
         test\r\n\
         --{boundary}\r\n\
         Content-Disposition: form-data; name=\"image\"; filename=\"test.png\"\r\n\
         Content-Type: image/png\r\n\r\n\
         fake\r\n\
         --{boundary}--\r\n"
    );

    let req = common::with_peer(axum::http::Request::builder(), "127.0.0.1:54321")
        .method("POST")
        .uri("/api/admin/emojis")
        .header("x-auth-consumer", "bob@test.example")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(axum::body::Body::from(body))
        .unwrap();

    let response = app.router.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../../migrations")]
async fn admin_delete_emoji(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool);
    let admin = common::seed_user_with_role(&app.state.db, "admin", "admin").await;

    // Create directly via DB
    let id = burst_core::id::new_id();
    burst_server::db::custom_emojis::create(
        &app.state.db,
        id,
        "deleteme",
        "emojis/test.png",
        admin.id,
    )
    .await
    .unwrap();

    let (status, _) = app
        .delete(&format!("/api/admin/emojis/{id}"), "admin@test.example")
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // Verify gone
    let (status, body) = app.get("/api/emojis", "admin@test.example").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["items"].as_array().unwrap().len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn shortcode_validation(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool);
    let _ = common::seed_user_with_role(&app.state.db, "admin", "admin").await;

    // Too short
    let boundary = "----b";
    let body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"shortcode\"\r\n\r\na\r\n\
         --{boundary}\r\nContent-Disposition: form-data; name=\"image\"; filename=\"t.png\"\r\n\
         Content-Type: image/png\r\n\r\ndata\r\n--{boundary}--\r\n"
    );
    let req = common::with_peer(axum::http::Request::builder(), "127.0.0.1:54321")
        .method("POST")
        .uri("/api/admin/emojis")
        .header("x-auth-consumer", "admin@test.example")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(axum::body::Body::from(body))
        .unwrap();
    let resp = app.router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}
