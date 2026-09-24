//! `@channel` and `@here`: who they mention and who they notify.

mod common;

use std::collections::HashSet;

use axum::http::StatusCode;
use burst_server::{db, ws::ServerEvent};
use uuid::Uuid;

fn auth(username: &str) -> String {
    format!("{username}@test.example")
}

struct Room {
    app: common::TestApp,
    pool: sqlx::PgPool,
    channel: Uuid,
    prefixed: String,
    alice: Uuid,
    bob: Uuid,
    carol: Uuid,
}

/// `general`, owned by alice, with bob and carol as members. Nobody is online
/// until a test says so.
async fn setup(pool: &sqlx::PgPool) -> Room {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(pool, "alice").await.id;
    let bob = common::seed_user(pool, "bob").await.id;
    let carol = common::seed_user(pool, "carol").await.id;
    let ch = common::seed_channel(pool, "general", alice).await;
    for user in [bob, carol] {
        db::channels::add_member(pool, ch.id, user, "member")
            .await
            .unwrap();
    }
    Room {
        app,
        pool: pool.clone(),
        channel: ch.id,
        prefixed: format!("ch_{}", ch.id),
        alice,
        bob,
        carol,
    }
}

impl Room {
    async fn online(&self, user: Uuid) {
        self.app.state.presence.connect(user).await;
    }

    async fn prefer(&self, user: Uuid, preference: &str) {
        db::channels::update_notify(&self.pool, self.channel, user, preference)
            .await
            .unwrap()
            .expect("member");
    }

    /// Sends as alice; returns who the message mentions and who was notified.
    async fn send(&self, content: &str) -> (HashSet<Uuid>, Vec<(Uuid, String)>) {
        let mut rx = self.app.state.broker.subscribe();
        let (status, body) = self
            .app
            .post(
                &format!("/api/channels/{}/messages", self.prefixed),
                &auth("alice"),
                serde_json::json!({ "content": content }),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        let message =
            burst_core::id::parse_prefixed_id(body["id"].as_str().unwrap(), "msg_").unwrap();
        (self.mentioned(message).await, notified(&mut rx))
    }

    async fn mentioned(&self, message: Uuid) -> HashSet<Uuid> {
        db::mentions::list_for_messages(&self.pool, &[message])
            .await
            .unwrap()
            .into_iter()
            .map(|(_, user)| user)
            .collect()
    }
}

fn notified(rx: &mut tokio::sync::broadcast::Receiver<ServerEvent>) -> Vec<(Uuid, String)> {
    let mut out = Vec::new();
    while let Ok(event) = rx.try_recv() {
        if let ServerEvent::NotificationCreated {
            recipient_id,
            reason,
            ..
        } = event
        {
            out.push((
                burst_core::id::parse_prefixed_id(&recipient_id, "usr_").unwrap(),
                reason,
            ));
        }
    }
    out
}

fn reason_for(notified: &[(Uuid, String)], user: Uuid) -> Option<&str> {
    notified
        .iter()
        .find(|(u, _)| *u == user)
        .map(|(_, r)| r.as_str())
}

// ── @channel ─────────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn channel_mentions_every_member_online_or_not(pool: sqlx::PgPool) {
    let room = setup(&pool).await;
    room.online(room.bob).await;

    let (mentioned, _) = room.send("@channel standup in five").await;
    assert_eq!(mentioned, HashSet::from([room.bob, room.carol]));
}

#[sqlx::test(migrations = "../../migrations")]
async fn channel_reaches_a_member_who_only_wants_mentions(pool: sqlx::PgPool) {
    let room = setup(&pool).await;
    room.prefer(room.bob, "mentions").await;

    let (_, notified) = room.send("@channel standup in five").await;
    assert_eq!(
        reason_for(&notified, room.bob),
        Some("mention"),
        "{notified:?}"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn channel_does_not_reach_a_member_who_muted_it(pool: sqlx::PgPool) {
    let room = setup(&pool).await;
    room.prefer(room.bob, "nothing").await;

    let (_, notified) = room.send("@channel standup in five").await;
    assert!(
        reason_for(&notified, room.carol).is_some(),
        "control: {notified:?}"
    );
    assert_eq!(reason_for(&notified, room.bob), None, "{notified:?}");
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_broadcast_does_not_mention_or_notify_its_author(pool: sqlx::PgPool) {
    let room = setup(&pool).await;
    room.online(room.alice).await;

    let (mentioned, notified) = room.send("@channel @here heads up").await;
    assert!(mentioned.contains(&room.bob), "control: {mentioned:?}");
    assert!(!mentioned.contains(&room.alice), "{mentioned:?}");
    assert_eq!(reason_for(&notified, room.alice), None, "{notified:?}");
}

// ── @here ────────────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn here_mentions_only_members_online_as_it_is_sent(pool: sqlx::PgPool) {
    let room = setup(&pool).await;
    room.online(room.bob).await;

    let (mentioned, _) = room.send("@here anyone free?").await;
    assert_eq!(mentioned, HashSet::from([room.bob]), "carol is offline");
}

#[sqlx::test(migrations = "../../migrations")]
async fn here_reaches_an_online_member_who_only_wants_mentions_and_not_an_offline_one(
    pool: sqlx::PgPool,
) {
    let room = setup(&pool).await;
    room.prefer(room.bob, "mentions").await;
    room.prefer(room.carol, "mentions").await;
    room.online(room.bob).await;

    let (_, notified) = room.send("@here anyone free?").await;
    assert_eq!(
        reason_for(&notified, room.bob),
        Some("mention"),
        "{notified:?}"
    );
    assert_eq!(reason_for(&notified, room.carol), None, "{notified:?}");
}

#[sqlx::test(migrations = "../../migrations")]
async fn here_with_nobody_online_mentions_no_one(pool: sqlx::PgPool) {
    let room = setup(&pool).await;
    let (mentioned, _) = room.send("@here anyone?").await;
    assert!(mentioned.is_empty(), "{mentioned:?}");
}

// ── Combined and elsewhere ───────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn a_broadcast_and_a_name_together_mention_both(pool: sqlx::PgPool) {
    let room = setup(&pool).await;
    room.online(room.bob).await;
    let dave = common::seed_user(&pool, "dave").await.id;

    // dave is named but not a member: mentioned, not notified.
    let (mentioned, notified) = room.send("@here and @dave").await;
    assert_eq!(mentioned, HashSet::from([room.bob, dave]));
    assert_eq!(reason_for(&notified, dave), None, "{notified:?}");
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_user_named_here_is_not_mentioned_by_name(pool: sqlx::PgPool) {
    let room = setup(&pool).await;
    let named_here = common::seed_user(&pool, "here").await.id;
    room.online(room.bob).await;

    let (mentioned, _) = room.send("@here quick question").await;
    assert!(mentioned.contains(&room.bob), "control: {mentioned:?}");
    assert!(
        !mentioned.contains(&named_here),
        "@here is a broadcast, not the user called 'here': {mentioned:?}"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_webhook_can_page_everyone_online(pool: sqlx::PgPool) {
    let room = setup(&pool).await;
    room.online(room.bob).await;
    let (wh, token) = common::seed_webhook(&pool, room.channel, "incoming", room.alice).await;

    let mut rx = room.app.state.broker.subscribe();
    let (status, body) = room
        .app
        .post_with_bearer(
            &format!(
                "/api/webhooks/{}/trigger",
                burst_core::id::format_webhook_id(wh.id)
            ),
            &token,
            serde_json::json!({ "content": "@here production is down" }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let message = burst_core::id::parse_prefixed_id(body["id"].as_str().unwrap(), "msg_").unwrap();

    assert_eq!(room.mentioned(message).await, HashSet::from([room.bob]));
    assert_eq!(reason_for(&notified(&mut rx), room.bob), Some("mention"));
}
