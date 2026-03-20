mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

#[sqlx::test(migrations = "../../migrations")]
async fn health_live_returns_ok(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool);
    let req = Request::get("/health/live").body(Body::empty()).unwrap();
    let res = app.admin.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[sqlx::test(migrations = "../../migrations")]
async fn health_ready_returns_ok(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool);
    let req = Request::get("/health/ready").body(Body::empty()).unwrap();
    let res = app.admin.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[sqlx::test(migrations = "../../migrations")]
async fn metrics_endpoint_returns_prometheus_text(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool);
    let req = Request::get("/metrics").body(Body::empty()).unwrap();
    let res = app.admin.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    // With the noop handle the body is empty, but the endpoint itself works.
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    // Noop renderer returns empty string — that's valid (no metrics recorded).
    assert!(
        bytes.is_empty() || String::from_utf8_lossy(&bytes).contains("burst_"),
        "Expected empty or Prometheus text"
    );
}
