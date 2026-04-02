mod common;

use axum::http::StatusCode;
use burst_server::ws::ServerEvent;

// ── POST /dms — happy path ────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn create_dm_returns_dm_channel(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let _alice = common::seed_user(&pool, "alice").await;
    let bob = common::seed_user(&pool, "bob").await;

    let (status, body) = app
        .post(
            "/api/dms",
            "alice@test.example",
            serde_json::json!({ "userId": format!("usr_{}", bob.id) }),
        )
        .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["kind"], "dm", "channel kind must be 'dm'");
    assert!(
        body["id"].as_str().unwrap_or("").starts_with("ch_"),
        "channel id must be prefixed with 'ch_'"
    );
}

/// Calling POST /dms twice for the same pair must return the same channel.
#[sqlx::test(migrations = "../../migrations")]
async fn create_dm_is_idempotent(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let _alice = common::seed_user(&pool, "alice").await;
    let bob = common::seed_user(&pool, "bob").await;

    let body = serde_json::json!({ "userId": format!("usr_{}", bob.id) });

    let (s1, first) = app
        .post("/api/dms", "alice@test.example", body.clone())
        .await;
    let (s2, second) = app.post("/api/dms", "alice@test.example", body).await;

    assert_eq!(s1, StatusCode::OK);
    assert_eq!(s2, StatusCode::OK);
    assert_eq!(
        first["id"], second["id"],
        "second call must return the same channel"
    );
}

/// The DM is also findable from bob's perspective (symmetric lookup).
#[sqlx::test(migrations = "../../migrations")]
async fn create_dm_is_symmetric(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await;
    let bob = common::seed_user(&pool, "bob").await;

    let (_, alice_view) = app
        .post(
            "/api/dms",
            "alice@test.example",
            serde_json::json!({ "userId": format!("usr_{}", bob.id) }),
        )
        .await;

    let (_, bob_view) = app
        .post(
            "/api/dms",
            "bob@test.example",
            serde_json::json!({ "userId": format!("usr_{}", alice.id) }),
        )
        .await;

    assert_eq!(
        alice_view["id"], bob_view["id"],
        "both users must see the same channel"
    );
}

// ── POST /dms — error cases ───────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn create_dm_with_self_is_bad_request(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await;

    let (status, _) = app
        .post(
            "/api/dms",
            "alice@test.example",
            serde_json::json!({ "userId": format!("usr_{}", alice.id) }),
        )
        .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../../migrations")]
async fn create_dm_with_unknown_user_is_not_found(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let _alice = common::seed_user(&pool, "alice").await;

    let ghost_id = uuid::Uuid::now_v7();
    let (status, _) = app
        .post(
            "/api/dms",
            "alice@test.example",
            serde_json::json!({ "userId": format!("usr_{ghost_id}") }),
        )
        .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../migrations")]
async fn create_dm_unauthenticated_is_unauthorized(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());

    let (status, _) = app
        .post_unauthenticated("/api/dms", serde_json::json!({ "userId": "usr_anything" }))
        .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ── POST /dms — WebSocket broadcast ──────────────────────────────────────────

/// This is the regression test for the original bug: after creating a DM,
/// `ChannelJoined` must be broadcast for BOTH participants so that open WS
/// connections update their `channel_ids` set and start receiving real-time
/// events on the new channel.
#[sqlx::test(migrations = "../../migrations")]
async fn create_dm_broadcasts_channel_joined_for_both_users(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await;
    let bob = common::seed_user(&pool, "bob").await;

    // Subscribe to the broker *before* the request so we don't miss any events.
    let mut rx = app.state.broker.subscribe();

    let (status, body) = app
        .post(
            "/api/dms",
            "alice@test.example",
            serde_json::json!({ "userId": format!("usr_{}", bob.id) }),
        )
        .await;

    assert_eq!(status, StatusCode::OK);
    let channel_id = body["id"]
        .as_str()
        .expect("response must have an id")
        .to_owned();

    // Drain all buffered broadcast events and collect ChannelJoined ones for
    // our new channel.
    let mut joined_for: Vec<String> = Vec::new();
    while let Ok(ev) = rx.try_recv() {
        if let ServerEvent::ChannelJoined {
            channel_id: ev_ch,
            user_id: ev_uid,
            ..
        } = ev
            && ev_ch == channel_id
        {
            joined_for.push(ev_uid);
        }
    }

    joined_for.sort();
    let mut expected = vec![format!("usr_{}", alice.id), format!("usr_{}", bob.id)];
    expected.sort();

    assert_eq!(
        joined_for, expected,
        "ChannelJoined must be broadcast for both participants so WS connections \
         add the new channel to their membership set"
    );
}
