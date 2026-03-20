mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use burst_server::config::{AppConfig, StorageConfig, WebSocketConfig};
use burst_server::{AppState, app_router, metrics};

/// Helper: create an AppState with a static_dir pointed at a temp directory.
fn app_with_static_dir(pool: sqlx::PgPool, dir: &std::path::Path) -> axum::Router {
    let storage_dir = tempfile::tempdir().unwrap();
    let config = AppConfig {
        storage: StorageConfig {
            local_path: storage_dir.path().to_string_lossy().into_owned(),
            ..StorageConfig::default()
        },
        websocket: WebSocketConfig::default(),
        server_static_dir: Some(dir.to_string_lossy().into_owned()),
    };
    let storage = burst_server::storage::Storage::Local(
        burst_server::storage::local::LocalStorage::new(storage_dir.path().to_path_buf()).unwrap(),
    );
    let state = AppState::new(
        pool,
        config,
        storage,
        metrics::noop(),
        tokio_util::sync::CancellationToken::new(),
    );
    app_router(state)
}

#[sqlx::test(migrations = "../../migrations")]
async fn static_serves_index_html(pool: sqlx::PgPool) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("index.html"), "<html>Burst</html>").unwrap();

    let router = app_with_static_dir(pool, dir.path());

    let req = Request::get("/static/index.html")
        .body(Body::empty())
        .unwrap();
    let res = router.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(body, "<html>Burst</html>");
}

#[sqlx::test(migrations = "../../migrations")]
async fn static_serves_nested_asset(pool: sqlx::PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let assets_dir = dir.path().join("assets");
    std::fs::create_dir_all(&assets_dir).unwrap();
    std::fs::write(assets_dir.join("main.js"), "console.log('burst')").unwrap();

    let router = app_with_static_dir(pool, dir.path());

    let req = Request::get("/static/assets/main.js")
        .body(Body::empty())
        .unwrap();
    let res = router.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(body, "console.log('burst')");
}

#[sqlx::test(migrations = "../../migrations")]
async fn static_spa_fallback_serves_index(pool: sqlx::PgPool) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("index.html"), "<html>SPA</html>").unwrap();

    let router = app_with_static_dir(pool, dir.path());

    // Request for a non-existent path should fall back to index.html (SPA routing)
    let req = Request::get("/static/login").body(Body::empty()).unwrap();
    let res = router.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(body, "<html>SPA</html>");
}

#[sqlx::test(migrations = "../../migrations")]
async fn static_spa_fallback_nested_path(pool: sqlx::PgPool) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("index.html"), "<html>SPA</html>").unwrap();

    let router = app_with_static_dir(pool, dir.path());

    // Deep nested SPA route should also fall back to index.html
    let req = Request::get("/static/channels/ch_123")
        .body(Body::empty())
        .unwrap();
    let res = router.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(body, "<html>SPA</html>");
}

#[sqlx::test(migrations = "../../migrations")]
async fn api_routes_still_work_with_static_dir(pool: sqlx::PgPool) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("index.html"), "<html>SPA</html>").unwrap();

    // Seed a user so auth works
    common::seed_user(&pool, "alice").await;

    let router = app_with_static_dir(pool, dir.path());

    // API routes should still work (not intercepted by static fallback)
    let req = Request::get("/emojis")
        .header("x-auth-consumer", "alice@test.example")
        .body(Body::empty())
        .unwrap();
    let res = router.clone().oneshot(req).await.unwrap();
    // Should get 200 (emoji list) not index.html
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(
        json["items"].is_array(),
        "expected JSON API response, not HTML"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn no_static_dir_returns_404_on_static_path(pool: sqlx::PgPool) {
    // When static_dir is not configured, /static/* should 404
    let app = common::TestApp::new(pool);

    let req = Request::get("/static/index.html")
        .body(Body::empty())
        .unwrap();
    let res = app.router.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}
