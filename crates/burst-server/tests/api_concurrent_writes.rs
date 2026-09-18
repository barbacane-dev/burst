//! Concurrent writes against the same resource.
//!
//! A serial test cannot see this class: the second request observes what the
//! first wrote. Every handler that reads, decides, then writes is a race, and
//! the first-login one (#111) shipped because nothing here ever ran two
//! requests at the same instant.
//!
//! The invariant is the same everywhere. Racing identical writes may refuse
//! some of them, but must never answer 5xx, and must leave the state a serial
//! run would have left.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::{TestApp, with_peer};
use serde_json::json;
use tower::ServiceExt;

fn auth(username: &str) -> String {
    format!("ext-{username}")
}

/// One request, returning the status and body.
async fn send(
    app: &TestApp,
    method: &str,
    uri: &str,
    external_id: &str,
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    let mut builder = with_peer(Request::builder(), "127.0.0.1:54321")
        .method(method)
        .uri(uri)
        .header("x-auth-consumer", external_id);
    let req = match body {
        Some(json) => {
            builder = builder.header("Content-Type", "application/json");
            builder
                .body(Body::from(serde_json::to_vec(&json).expect("json")))
                .expect("request")
        }
        None => builder.body(Body::empty()).expect("request"),
    };
    let res = app.router.clone().oneshot(req).await.expect("response");
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("body");
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    )
}

/// Run `n` identical requests at once, returning every status.
async fn race(
    pool: &sqlx::PgPool,
    n: usize,
    method: &'static str,
    uri: String,
    external_id: String,
    body: Option<serde_json::Value>,
) -> Vec<StatusCode> {
    let mut set = tokio::task::JoinSet::new();
    for _ in 0..n {
        let app = TestApp::new(pool.clone());
        let (uri, eid, body) = (uri.clone(), external_id.clone(), body.clone());
        set.spawn(async move { send(&app, method, &uri, &eid, body).await.0 });
    }
    let mut out = Vec::new();
    while let Some(r) = set.join_next().await {
        out.push(r.expect("task"));
    }
    out
}

fn assert_no_server_errors(codes: &[StatusCode], what: &str) {
    let server_errors: Vec<_> = codes.iter().filter(|c| c.is_server_error()).collect();
    assert!(
        server_errors.is_empty(),
        "{what}: racing identical writes must not 5xx, got {codes:?}"
    );
}

/// Creating the same channel twice at once. One wins; the other is refused for
/// the slug it wanted, not with a constraint violation escaping as a 500.
#[sqlx::test(migrations = "../../migrations")]
async fn creating_one_channel_twice_at_once(pool: sqlx::PgPool) {
    common::seed_user(&pool, "alice").await;
    let body = json!({ "name": "race-room", "kind": "public" });

    let codes = race(
        &pool,
        8,
        "POST",
        "/api/channels".into(),
        auth("alice"),
        Some(body),
    )
    .await;
    assert_no_server_errors(&codes, "channel create");

    let created = codes.iter().filter(|c| c.is_success()).count();
    assert_eq!(created, 1, "exactly one creator wins, got {codes:?}");

    // And the slug is not duplicated.
    let app = TestApp::new(pool.clone());
    let (status, body) = send(&app, "GET", "/api/channels", &auth("alice"), None).await;
    assert_eq!(status, StatusCode::OK);
    let items = body["items"].as_array().cloned().unwrap_or_default();
    let matching = items.iter().filter(|c| c["name"] == "race-room").count();
    assert_eq!(matching, 1, "one channel exists, not {matching}");
}

/// Joining the same channel repeatedly at once. Membership is keyed on
/// (channel, user), so the second insert collides.
#[sqlx::test(migrations = "../../migrations")]
async fn joining_one_channel_many_times_at_once(pool: sqlx::PgPool) {
    common::seed_user(&pool, "alice").await;
    common::seed_user(&pool, "bob").await;

    let app = TestApp::new(pool.clone());
    let (status, channel) = send(
        &app,
        "POST",
        "/api/channels",
        &auth("alice"),
        Some(json!({ "name": "join-room", "kind": "public" })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CREATED.max(status),
        "created: {channel}"
    );
    let id = channel["id"].as_str().expect("channel id").to_string();

    let codes = race(
        &pool,
        8,
        "POST",
        format!("/api/channels/{id}/members"),
        auth("bob"),
        None,
    )
    .await;
    assert_no_server_errors(&codes, "channel join");

    // However many were refused, bob is a member exactly once.
    let app = TestApp::new(pool.clone());
    let (status, members) = send(
        &app,
        "GET",
        &format!("/api/channels/{id}/members"),
        &auth("alice"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let items = members["items"].as_array().cloned().unwrap_or_default();
    let bobs = items
        .iter()
        .filter(|m| m["username"] == "bob" || m["user"]["username"] == "bob")
        .count();
    assert!(bobs <= 1, "bob joined {bobs} times: {members}");
}

/// Posting at once is not a uniqueness race, but it is the hottest write path
/// and must not lose or duplicate anything under contention.
#[sqlx::test(migrations = "../../migrations")]
async fn posting_many_messages_at_once(pool: sqlx::PgPool) {
    common::seed_user(&pool, "alice").await;

    let app = TestApp::new(pool.clone());
    let (_, channel) = send(
        &app,
        "POST",
        "/api/channels",
        &auth("alice"),
        Some(json!({ "name": "busy-room", "kind": "public" })),
    )
    .await;
    let id = channel["id"].as_str().expect("channel id").to_string();

    const N: usize = 12;
    let mut set = tokio::task::JoinSet::new();
    for i in 0..N {
        let app = TestApp::new(pool.clone());
        let uri = format!("/api/channels/{id}/messages");
        set.spawn(async move {
            send(
                &app,
                "POST",
                &uri,
                &auth("alice"),
                Some(json!({ "content": format!("message {i}") })),
            )
            .await
            .0
        });
    }
    let mut codes = Vec::new();
    while let Some(r) = set.join_next().await {
        codes.push(r.expect("task"));
    }
    assert_no_server_errors(&codes, "message post");
    assert!(
        codes.iter().all(|c| c.is_success()),
        "every post should succeed, got {codes:?}"
    );

    let app = TestApp::new(pool.clone());
    let (status, listed) = send(
        &app,
        "GET",
        &format!("/api/channels/{id}/messages?limit=50"),
        &auth("alice"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let items = listed["items"].as_array().cloned().unwrap_or_default();
    assert_eq!(items.len(), N, "every message is stored exactly once");
}
