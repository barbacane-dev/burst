//! What a connected socket receives as its user's memberships change.
//!
//! These run a real server on a real port with real WebSocket clients, since
//! the property is about the socket's own loop: which channels it forwards,
//! and when that set changes. Unit tests of the helpers cannot show the loop
//! calls them.

mod common;

use std::time::Duration;

use axum::http::StatusCode;
use burst_server::db;
use serde_json::Value;
use uuid::Uuid;

const ARRIVES: Duration = Duration::from_secs(3);
/// How long to go on listening for something that must not arrive, once a
/// control socket has shown the same broadcast has already been delivered.
const STAYS_AWAY: Duration = Duration::from_millis(500);

fn auth(username: &str) -> String {
    format!("{username}@test.example")
}

fn ch(id: Uuid) -> String {
    burst_core::id::format_channel_id(id)
}

async fn private_channel(pool: &sqlx::PgPool, owner: Uuid) -> Uuid {
    let id = burst_core::id::new_id();
    db::channels::create(
        pool,
        &db::channels::CreateChannel {
            id,
            kind: "private".into(),
            name: Some("secret".into()),
            slug: Some("secret".into()),
            topic: None,
            description: None,
            created_by: owner,
        },
    )
    .await
    .unwrap();
    db::channels::add_member(pool, id, owner, "owner")
        .await
        .unwrap();
    id
}

async fn say(app: &common::TestApp, channel: Uuid, as_user: &str, text: &str) {
    let (status, body) = app
        .post(
            &format!("/api/channels/{}/messages", ch(channel)),
            &auth(as_user),
            serde_json::json!({ "content": text }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
}

/// A channel both users are already in, and a message there that proves the
/// socket is subscribed. A socket announces its own presence before it
/// subscribes, so it never sees that; a message is the dependable signal.
async fn ready(
    app: &common::TestApp,
    pool: &sqlx::PgPool,
    owner: Uuid,
    member: Uuid,
    socket: &mut common::Socket,
) {
    let lobby = common::seed_channel(pool, "lobby", owner).await.id;
    db::channels::add_member(pool, lobby, member, "member")
        .await
        .unwrap();
    // The socket loaded its memberships before this channel existed, so it
    // learns of it the way any added member does.
    app.state
        .broker
        .publish(burst_server::ws::ServerEvent::ChannelJoined {
            event_id: burst_core::id::new_id().to_string(),
            channel_id: ch(lobby),
            user_id: burst_core::id::format_user_id(member),
        });
    for attempt in 0..20 {
        say(app, lobby, "alice", &format!("ready {attempt}")).await;
        if common::wait_for(socket, Duration::from_millis(200), |e| {
            e["type"] == "message.created"
                && e["message"]["content"]
                    .as_str()
                    .is_some_and(|c| c.starts_with("ready"))
        })
        .await
        .is_some()
        {
            return;
        }
    }
    panic!("the socket never became ready");
}

fn message_saying(text: &'static str) -> impl Fn(&Value) -> bool {
    move |e| e["type"] == "message.created" && e["message"]["content"] == text
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_removed_member_stops_receiving_the_channel_live(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await.id;
    let bob = common::seed_user(&pool, "bob").await.id;
    let carol = common::seed_user(&pool, "carol").await.id;
    let room = private_channel(&pool, alice).await;
    for user in [bob, carol] {
        db::channels::add_member(&pool, room, user, "member")
            .await
            .unwrap();
    }
    let addr = app.serve().await;
    let mut bobs = common::connect(addr, &auth("bob")).await;
    let mut carols = common::connect(addr, &auth("carol")).await;

    // Both sockets are subscribed once they have seen a message.
    say(&app, room, "alice", "warm-up").await;
    assert!(
        common::wait_for(&mut bobs, ARRIVES, message_saying("warm-up"))
            .await
            .is_some()
    );
    assert!(
        common::wait_for(&mut carols, ARRIVES, message_saying("warm-up"))
            .await
            .is_some()
    );

    let (status, _) = app
        .delete(
            &format!(
                "/api/channels/{}/members/{}",
                ch(room),
                burst_core::id::format_user_id(bob)
            ),
            &auth("alice"),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(
        common::wait_for(&mut bobs, ARRIVES, |e| e["type"] == "channel.left")
            .await
            .is_some(),
        "bob should be told he left"
    );

    say(&app, room, "alice", "after bob left").await;
    assert!(
        common::wait_for(&mut carols, ARRIVES, message_saying("after bob left"))
            .await
            .is_some(),
        "control: carol still receives the channel"
    );
    assert!(
        common::wait_for(&mut bobs, STAYS_AWAY, message_saying("after bob left"))
            .await
            .is_none(),
        "bob was removed and still received the channel's messages"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_member_added_while_connected_starts_receiving_without_reconnecting(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await.id;
    let bob = common::seed_user(&pool, "bob").await.id;
    let room = private_channel(&pool, alice).await;
    let addr = app.serve().await;
    let mut bobs = common::connect(addr, &auth("bob")).await;
    ready(&app, &pool, alice, bob, &mut bobs).await;

    let (status, _) = app
        .post(
            &format!("/api/channels/{}/members", ch(room)),
            &auth("alice"),
            serde_json::json!({ "userId": burst_core::id::format_user_id(bob) }),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    say(&app, room, "alice", "welcome, bob").await;
    assert!(
        common::wait_for(&mut bobs, ARRIVES, message_saying("welcome, bob"))
            .await
            .is_some(),
        "bob was added and did not receive the channel until reconnecting"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn joining_a_public_channel_while_connected_delivers_it_straight_away(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await.id;
    let bob = common::seed_user(&pool, "bob").await.id;
    let general = common::seed_channel(&pool, "general", alice).await.id;
    let addr = app.serve().await;
    let mut bobs = common::connect(addr, &auth("bob")).await;
    ready(&app, &pool, alice, bob, &mut bobs).await;

    let (status, _) = app
        .post(
            &format!("/api/channels/{}/members", ch(general)),
            &auth("bob"),
            serde_json::Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    say(&app, general, "alice", "hi bob").await;
    assert!(
        common::wait_for(&mut bobs, ARRIVES, message_saying("hi bob"))
            .await
            .is_some(),
        "bob joined and did not receive the channel until reconnecting"
    );
}
