mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::{TestApp, with_peer};
use tower::ServiceExt;

/// Drive one request for `external_id` through the router.
async fn call(app: &TestApp, external_id: &str) -> StatusCode {
    let req = with_peer(Request::builder(), "127.0.0.1:54321")
        .method("GET")
        .uri("/api/users/me")
        .header("x-auth-consumer", external_id)
        .body(Body::empty())
        .expect("request");
    app.router
        .clone()
        .oneshot(req)
        .await
        .expect("response")
        .status()
}

/// The first request from an identity provisions it, and a browser makes
/// several at once as it loads. Before this, each one found no user, each tried
/// to create it, one won and the rest hit the unique constraint, which surfaced
/// as 409. A serial test cannot see it: the second request finds the user the
/// first created.
#[sqlx::test(migrations = "../../migrations")]
async fn concurrent_first_requests_all_succeed(pool: sqlx::PgPool) {
    let app = TestApp::new(pool.clone());
    let external_id = "a1b2c3d4-0000-0000-0000-0000000000aa";

    let mut set = tokio::task::JoinSet::new();
    for _ in 0..16 {
        let app = TestApp::new(pool.clone());
        let id = external_id.to_string();
        set.spawn(async move { call(&app, &id).await });
    }

    let mut codes = Vec::new();
    while let Some(res) = set.join_next().await {
        codes.push(res.expect("task"));
    }

    let ok = codes.iter().filter(|c| **c == StatusCode::OK).count();
    assert_eq!(
        ok,
        codes.len(),
        "every concurrent first request must succeed, got {codes:?}"
    );

    // Exactly one user, whichever request won the insert.
    let found = burst_server::db::users::find_by_external_id(&pool, external_id)
        .await
        .expect("query");
    assert!(found.is_some(), "the identity was provisioned");

    // And the app still works afterwards.
    assert_eq!(call(&app, external_id).await, StatusCode::OK);
}

/// Two different identities racing must not be confused for one another.
#[sqlx::test(migrations = "../../migrations")]
async fn concurrent_distinct_identities_each_get_their_own_user(pool: sqlx::PgPool) {
    let ids: Vec<String> = (0..8)
        .map(|i| format!("a1b2c3d4-0000-0000-0000-0000000000{i:02x}"))
        .collect();

    let mut set = tokio::task::JoinSet::new();
    for id in ids.clone() {
        let app = TestApp::new(pool.clone());
        set.spawn(async move { call(&app, &id).await });
    }
    while let Some(res) = set.join_next().await {
        assert_eq!(res.expect("task"), StatusCode::OK);
    }

    let mut user_ids = Vec::new();
    for id in &ids {
        let user = burst_server::db::users::find_by_external_id(&pool, id)
            .await
            .expect("query")
            .expect("provisioned");
        user_ids.push(user.id);
    }
    user_ids.sort();
    user_ids.dedup();
    assert_eq!(user_ids.len(), ids.len(), "each identity got its own user");
}
