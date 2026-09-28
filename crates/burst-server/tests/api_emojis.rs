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

/// A 1×1 GIF, so the bytes served back can be compared exactly.
const GIF: &[u8] = b"GIF89a\x01\x00\x01\x00\x80\x00\x00\xff\xff\xff\x00\x00\x00!\xf9\x04\x01\x00\x00\x00\x00,\x00\x00\x00\x00\x01\x00\x01\x00\x00\x02\x02D\x01\x00;";

/// Uploads an emoji as `admin@test.example` and returns the API's response.
async fn upload_emoji(app: &common::TestApp, shortcode: &str, image: &[u8]) -> serde_json::Value {
    let boundary = "----boundary";
    let mut body = format!(
        "--{boundary}\r\n\
         Content-Disposition: form-data; name=\"shortcode\"\r\n\r\n\
         {shortcode}\r\n\
         --{boundary}\r\n\
         Content-Disposition: form-data; name=\"image\"; filename=\"{shortcode}.gif\"\r\n\
         Content-Type: image/gif\r\n\r\n"
    )
    .into_bytes();
    body.extend_from_slice(image);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

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
    serde_json::from_slice(&bytes).unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
async fn an_uploaded_emoji_is_served_at_the_url_the_api_gives(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool);
    let _ = common::seed_user_with_role(&app.state.db, "admin", "admin").await;
    let _ = common::seed_user(&app.state.db, "alice").await;

    let emoji = upload_emoji(&app, "tinygif", GIF).await;
    let url = emoji["imageUrl"].as_str().expect("an imageUrl");
    assert_eq!(
        url,
        format!("/api/emojis/{}/image", emoji["id"].as_str().unwrap())
    );

    // The list gives the same URL.
    let (_, list) = app.get("/api/emojis", "alice@test.example").await;
    assert_eq!(list["items"][0]["imageUrl"], url);

    // Any member loads it, byte for byte.
    let (status, headers, body) = app.get_raw(url, "alice@test.example").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(&body[..], GIF);
    assert_eq!(headers["content-type"], "image/gif");
    assert_eq!(headers["x-content-type-options"], "nosniff");
    assert!(
        headers["content-security-policy"]
            .to_str()
            .unwrap()
            .contains("sandbox"),
        "an SVG opened directly must not run script"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn an_unknown_or_malformed_emoji_has_no_image(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool);
    let _ = common::seed_user(&app.state.db, "alice").await;

    let unknown = format!("/api/emojis/{}/image", burst_core::id::new_id());
    let (status, _, _) = app.get_raw(&unknown, "alice@test.example").await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _, _) = app
        .get_raw("/api/emojis/not-an-id/image", "alice@test.example")
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../../migrations")]
async fn deleting_an_emoji_removes_its_image(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool);
    let _ = common::seed_user_with_role(&app.state.db, "admin", "admin").await;

    let emoji = upload_emoji(&app, "shortlived", GIF).await;
    let id = emoji["id"].as_str().unwrap();
    let url = emoji["imageUrl"].as_str().unwrap().to_string();
    let key = format!("emojis/{id}.gif");
    assert!(
        app.state.storage.get(&key).await.is_ok(),
        "stored on upload"
    );

    let (status, _) = app
        .delete(&format!("/api/admin/emojis/{id}"), "admin@test.example")
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, _, _) = app.get_raw(&url, "admin@test.example").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(
        app.state.storage.get(&key).await.is_err(),
        "the file goes with the emoji"
    );
}
