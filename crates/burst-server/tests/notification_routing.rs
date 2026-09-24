//! Who is notified of a new message, observed on the broker.
//!
//! The preference tests elsewhere only check that the PATCH is accepted. These
//! check that it changes who hears about a message.

mod common;

use axum::http::StatusCode;
use burst_server::{db, ws::ServerEvent};
use uuid::Uuid;

fn auth(username: &str) -> String {
    format!("{username}@test.example")
}

struct Channel {
    app: common::TestApp,
    id: Uuid,
    prefixed: String,
}

/// A channel `general` owned by alice, with bob and carol as members.
async fn setup(pool: &sqlx::PgPool) -> (Channel, Uuid, Uuid, Uuid) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(pool, "alice").await;
    let bob = common::seed_user(pool, "bob").await;
    let carol = common::seed_user(pool, "carol").await;
    let ch = common::seed_channel(pool, "general", alice.id).await;
    for user in [bob.id, carol.id] {
        db::channels::add_member(pool, ch.id, user, "member")
            .await
            .unwrap();
    }
    let prefixed = format!("ch_{}", ch.id);
    (
        Channel {
            app,
            id: ch.id,
            prefixed,
        },
        alice.id,
        bob.id,
        carol.id,
    )
}

async fn set_preference(pool: &sqlx::PgPool, channel: Uuid, user: Uuid, preference: &str) {
    db::channels::update_notify(pool, channel, user, preference)
        .await
        .unwrap()
        .expect("the user is a member");
}

/// A notification observed on the broker, with its recipient parsed.
#[derive(Debug)]
struct Seen {
    recipient: Uuid,
    reason: String,
    event: ServerEvent,
}

/// Sends `content` as `sender` and returns the notifications it produced.
async fn send(channel: &Channel, sender: &str, content: &str) -> Vec<Seen> {
    let mut rx = channel.app.state.broker.subscribe();
    let (status, body) = channel
        .app
        .post(
            &format!("/api/channels/{}/messages", channel.prefixed),
            &auth(sender),
            serde_json::json!({ "content": content }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    drain(&mut rx)
}

fn drain(rx: &mut tokio::sync::broadcast::Receiver<ServerEvent>) -> Vec<Seen> {
    let mut seen = Vec::new();
    while let Ok(event) = rx.try_recv() {
        if let ServerEvent::NotificationCreated {
            recipient_id,
            reason,
            ..
        } = &event
        {
            seen.push(Seen {
                recipient: burst_core::id::parse_prefixed_id(recipient_id, "usr_").unwrap(),
                reason: reason.clone(),
                event: event.clone(),
            });
        }
    }
    seen
}

/// Guards an absence assertion: with no notifications at all, "X was not
/// notified" holds trivially, so each such test also proves someone was.
fn assert_notified(seen: &[Seen], user: Uuid) {
    assert!(
        seen.iter().any(|s| s.recipient == user),
        "routing produced no notification for the control member: {seen:?}"
    );
}

fn recipients(seen: &[Seen]) -> Vec<Uuid> {
    let mut ids: Vec<_> = seen.iter().map(|s| s.recipient).collect();
    ids.sort();
    ids
}

// ── Preferences ──────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn the_author_is_not_notified_of_their_own_message(pool: sqlx::PgPool) {
    let (channel, alice, bob, _) = setup(&pool).await;
    let seen = send(&channel, "alice", "hello").await;
    assert_notified(&seen, bob);
    assert!(
        !recipients(&seen).contains(&alice),
        "alice was notified of her own message: {seen:?}"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn members_left_at_the_default_hear_every_message(pool: sqlx::PgPool) {
    let (channel, _, bob, carol) = setup(&pool).await;
    let seen = send(&channel, "alice", "hello").await;

    let mut expected = vec![bob, carol];
    expected.sort();
    assert_eq!(recipients(&seen), expected);
    assert!(seen.iter().all(|s| s.reason == "message"), "{seen:?}");
}

#[sqlx::test(migrations = "../../migrations")]
async fn mentions_only_hears_a_mention_and_nothing_else(pool: sqlx::PgPool) {
    let (channel, _, bob, carol) = setup(&pool).await;
    set_preference(&pool, channel.id, bob, "mentions").await;

    let plain = send(&channel, "alice", "hello").await;
    assert_notified(&plain, carol);
    assert!(!recipients(&plain).contains(&bob), "{plain:?}");

    let mention = send(&channel, "alice", "hey @bob").await;
    let bobs: Vec<_> = mention.iter().filter(|s| s.recipient == bob).collect();
    assert_eq!(bobs.len(), 1, "{mention:?}");
    assert_eq!(bobs[0].reason, "mention");
}

#[sqlx::test(migrations = "../../migrations")]
async fn nothing_is_silent_even_when_mentioned(pool: sqlx::PgPool) {
    let (channel, _, bob, carol) = setup(&pool).await;
    set_preference(&pool, channel.id, bob, "nothing").await;

    let seen = send(&channel, "alice", "hey @bob, urgent").await;
    assert_notified(&seen, carol);
    assert!(!recipients(&seen).contains(&bob), "{seen:?}");
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_mention_is_the_reason_given_to_a_member_hearing_everything(pool: sqlx::PgPool) {
    let (channel, _, bob, carol) = setup(&pool).await;
    let seen = send(&channel, "alice", "@bob look").await;

    let reason_for = |user| {
        seen.iter()
            .find(|s| s.recipient == user)
            .map(|s| s.reason.as_str())
    };
    assert_eq!(reason_for(bob), Some("mention"));
    assert_eq!(reason_for(carol), Some("message"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_preference_set_through_the_api_changes_who_is_notified(pool: sqlx::PgPool) {
    let (channel, _, bob, carol) = setup(&pool).await;

    let (status, _) = channel
        .app
        .patch(
            &format!("/api/channels/{}/members/me/notify", channel.prefixed),
            &auth("bob"),
            serde_json::json!({ "notify": "nothing" }),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let seen = send(&channel, "alice", "hello").await;
    assert_notified(&seen, carol);
    assert!(
        !recipients(&seen).contains(&bob),
        "bob muted the channel through the API and was still notified: {seen:?}"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn mentioning_a_non_member_notifies_no_one_outside_the_channel(pool: sqlx::PgPool) {
    let (channel, _, bob, _) = setup(&pool).await;
    let dave = common::seed_user(&pool, "dave").await;

    let seen = send(&channel, "alice", "@dave are you there").await;
    assert_notified(&seen, bob);
    assert!(!recipients(&seen).contains(&dave.id), "{seen:?}");
}

// ── What a notification carries ──────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn a_notification_names_the_author_and_channel_and_previews_the_text(pool: sqlx::PgPool) {
    let (channel, _, bob, _) = setup(&pool).await;
    let seen = send(&channel, "alice", "the build is green").await;

    let for_bob = seen
        .iter()
        .find(|s| s.recipient == bob)
        .expect("bob notified");
    let ServerEvent::NotificationCreated {
        author_name,
        channel_name,
        preview,
        channel_id,
        ..
    } = &for_bob.event
    else {
        unreachable!()
    };
    assert_eq!(author_name, "alice");
    assert_eq!(channel_name.as_deref(), Some("general"));
    assert_eq!(preview, "the build is green");
    assert_eq!(channel_id, &channel.prefixed);
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_notification_does_not_carry_an_event_id(pool: sqlx::PgPool) {
    // The client adopts any `eventId` as its gap-fill cursor. Notifications
    // are not buffered, so one carrying an `eventId` would make the next
    // reconnect ask for an id the buffer never held and force a full refetch.
    let (channel, _, bob, _) = setup(&pool).await;
    let seen = send(&channel, "alice", "hello").await;
    let for_bob = seen
        .iter()
        .find(|s| s.recipient == bob)
        .expect("bob notified");

    let json = serde_json::to_value(&for_bob.event).unwrap();
    assert_eq!(json["type"], "notification.created");
    assert!(json.get("eventId").is_none(), "{json}");
    assert!(json["notificationId"].is_string(), "{json}");
}

#[sqlx::test(migrations = "../../migrations")]
async fn notifications_are_not_replayed_on_reconnect(pool: sqlx::PgPool) {
    let (channel, _, bob, _) = setup(&pool).await;
    let before = burst_core::id::new_id();
    channel
        .app
        .state
        .event_buffer
        .push(
            before,
            ServerEvent::ChannelUpdated {
                event_id: before.to_string(),
                channel_id: channel.prefixed.clone(),
            },
        )
        .await;

    let seen = send(&channel, "alice", "hello").await;
    assert_notified(&seen, bob);

    let replayed = channel
        .app
        .state
        .event_buffer
        .events_since(before)
        .await
        .expect("the marker is still buffered");
    assert!(
        replayed
            .iter()
            .all(|e| !matches!(e, ServerEvent::NotificationCreated { .. })),
        "a notification entered the gap-fill buffer: {replayed:?}"
    );
    assert!(
        replayed
            .iter()
            .any(|e| matches!(e, ServerEvent::MessageCreated { .. })),
        "the message itself should still be replayed"
    );
}

// ── Incoming webhooks ────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn a_webhook_post_notifies_members_and_honours_mentions(pool: sqlx::PgPool) {
    let (channel, alice, bob, carol) = setup(&pool).await;
    set_preference(&pool, channel.id, carol, "mentions").await;
    let (wh, token) = common::seed_webhook(&pool, channel.id, "incoming", alice).await;
    let wh_id = burst_core::id::format_webhook_id(wh.id);

    let mut rx = channel.app.state.broker.subscribe();
    let (status, body) = channel
        .app
        .post_with_bearer(
            &format!("/api/webhooks/{wh_id}/trigger"),
            &token,
            serde_json::json!({ "content": "deploy finished, @carol please verify" }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let seen = drain(&mut rx);

    let reason_for = |user| {
        seen.iter()
            .find(|s| s.recipient == user)
            .map(|s| s.reason.as_str())
    };
    assert_eq!(reason_for(bob), Some("message"), "{seen:?}");
    assert_eq!(reason_for(carol), Some("mention"), "{seen:?}");
    // The webhook posts as its creator, so the creator is the author.
    assert_eq!(reason_for(alice), None, "{seen:?}");
}
