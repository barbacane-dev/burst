mod common;

use axum::http::StatusCode;
use burst_server::db;

// ── DB: find_or_create_dm ─────────────────────────────────────────────────────

/// Both users must appear in channel_members after the DM is created.
/// This is the fundamental invariant that the WS handler relies on when
/// loading `channel_ids` at connect time.
#[sqlx::test(migrations = "../../migrations")]
async fn find_or_create_dm_adds_both_users_as_members(pool: sqlx::PgPool) {
    let alice = common::seed_user(&pool, "alice").await;
    let bob = common::seed_user(&pool, "bob").await;

    let channel =
        db::channels::find_or_create_dm(&pool, alice.id, bob.id, burst_core::id::new_id())
            .await
            .unwrap();

    let members = db::channels::list_members(&pool, channel.id).await.unwrap();
    let member_ids: Vec<uuid::Uuid> = members.iter().map(|m| m.user_id).collect();

    assert_eq!(members.len(), 2, "DM channel must have exactly 2 members");
    assert!(member_ids.contains(&alice.id), "alice must be a member");
    assert!(member_ids.contains(&bob.id), "bob must be a member");
}

/// find_or_create_dm must return the same channel on repeat calls.
#[sqlx::test(migrations = "../../migrations")]
async fn find_or_create_dm_is_idempotent(pool: sqlx::PgPool) {
    let alice = common::seed_user(&pool, "alice").await;
    let bob = common::seed_user(&pool, "bob").await;

    let ch1 = db::channels::find_or_create_dm(&pool, alice.id, bob.id, burst_core::id::new_id())
        .await
        .unwrap();

    let ch2 = db::channels::find_or_create_dm(&pool, alice.id, bob.id, burst_core::id::new_id())
        .await
        .unwrap();

    assert_eq!(ch1.id, ch2.id, "second call must return the same channel");
}

/// The lookup must work regardless of argument order.
#[sqlx::test(migrations = "../../migrations")]
async fn find_or_create_dm_is_commutative(pool: sqlx::PgPool) {
    let alice = common::seed_user(&pool, "alice").await;
    let bob = common::seed_user(&pool, "bob").await;

    let ch_ab = db::channels::find_or_create_dm(&pool, alice.id, bob.id, burst_core::id::new_id())
        .await
        .unwrap();

    let ch_ba = db::channels::find_or_create_dm(&pool, bob.id, alice.id, burst_core::id::new_id())
        .await
        .unwrap();

    assert_eq!(ch_ab.id, ch_ba.id, "lookup must be symmetric");
}

// ── API: channel membership guards ───────────────────────────────────────────

/// Non-members must receive 403 when attempting to send a message.
#[sqlx::test(migrations = "../../migrations")]
async fn non_member_cannot_send_message(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await;
    let eve = common::seed_user(&pool, "eve").await;

    // Alice creates a channel (becomes owner/member).
    let (status, ch) = app
        .post(
            "/channels",
            &app.token(alice.id, "member"),
            serde_json::json!({ "name": "alices-room" }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let channel_id = ch["id"].as_str().unwrap().to_owned();

    // Eve (non-member) attempts to post.
    let (status, _) = app
        .post(
            &format!("/channels/{channel_id}/messages"),
            &app.token(eve.id, "member"),
            serde_json::json!({ "content": "you shall not post" }),
        )
        .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

/// A joined member must be able to send messages.
#[sqlx::test(migrations = "../../migrations")]
async fn member_can_send_message(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await;
    let bob = common::seed_user(&pool, "bob").await;

    // Alice creates the channel.
    let (_, ch) = app
        .post(
            "/channels",
            &app.token(alice.id, "member"),
            serde_json::json!({ "name": "general" }),
        )
        .await;
    let channel_id = ch["id"].as_str().unwrap().to_owned();

    // Bob joins.
    let (join_status, _) = app
        .post(
            &format!("/channels/{channel_id}/members"),
            &app.token(bob.id, "member"),
            serde_json::Value::Null,
        )
        .await;
    assert_eq!(join_status, StatusCode::NO_CONTENT);

    // Bob sends a message.
    let (status, msg) = app
        .post(
            &format!("/channels/{channel_id}/messages"),
            &app.token(bob.id, "member"),
            serde_json::json!({ "content": "hello!" }),
        )
        .await;

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(msg["content"], "hello!");
}

// ── API: GET /channels — unread count semantics ───────────────────────────────

/// A message sent by the author must not increment their own unread count.
#[sqlx::test(migrations = "../../migrations")]
async fn own_message_does_not_count_as_unread(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await;
    let ch = common::seed_channel(&pool, "general", alice.id).await;

    common::seed_message(&pool, ch.id, alice.id, "hello from alice").await;

    let (status, list) = app
        .get("/channels?joined=true", &app.token(alice.id, "member"))
        .await;
    assert_eq!(status, StatusCode::OK);

    let ch_api_id = burst_core::id::format_channel_id(ch.id);
    let channel = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"].as_str() == Some(&ch_api_id))
        .expect("channel must appear in alice's list");

    assert_eq!(
        channel["unreadCount"].as_i64().unwrap_or(-1),
        0,
        "alice's own message must not appear as unread"
    );
}

/// A message sent by a peer must increment the other member's unread count.
#[sqlx::test(migrations = "../../migrations")]
async fn peer_message_counts_as_unread_for_other_member(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await;
    let bob = common::seed_user(&pool, "bob").await;
    let ch = common::seed_channel(&pool, "general", alice.id).await;

    burst_server::db::channels::add_member(&pool, ch.id, bob.id, "member")
        .await
        .unwrap();

    common::seed_message(&pool, ch.id, bob.id, "hello from bob").await;

    let (status, list) = app
        .get("/channels?joined=true", &app.token(alice.id, "member"))
        .await;
    assert_eq!(status, StatusCode::OK);

    let ch_api_id = burst_core::id::format_channel_id(ch.id);
    let channel = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"].as_str() == Some(&ch_api_id))
        .expect("channel must appear in alice's list");

    assert_eq!(
        channel["unreadCount"].as_i64().unwrap_or(-1),
        1,
        "bob's message must count as unread for alice"
    );
}

// ── API: GET /channels — DM appears in user's channel list ───────────────────

/// After creating a DM, it must be visible in GET /channels for both users.
#[sqlx::test(migrations = "../../migrations")]
async fn dm_appears_in_channel_list_for_both_participants(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await;
    let bob = common::seed_user(&pool, "bob").await;

    let (status, dm) = app
        .post(
            "/dms",
            &app.token(alice.id, "member"),
            serde_json::json!({ "userId": format!("usr_{}", bob.id) }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let dm_id = dm["id"].as_str().unwrap().to_owned();

    for (label, user) in [("alice", &alice), ("bob", &bob)] {
        let (s, list) = app
            .get("/channels?joined=true", &app.token(user.id, "member"))
            .await;
        assert_eq!(s, StatusCode::OK, "{label}: GET /channels failed");
        let ids: Vec<&str> = list["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|c| c["id"].as_str())
            .collect();
        assert!(
            ids.contains(&dm_id.as_str()),
            "{label}'s channel list must contain the DM channel"
        );
    }
}
