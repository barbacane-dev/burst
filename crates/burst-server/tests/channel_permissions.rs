//! Membership and moderation: adding people to channels, what guests may do,
//! what moderators may do, removing members and appointing moderators.
//!
//! Every refusal here is paired with the same request succeeding for someone
//! allowed to make it, so a test cannot pass because the endpoint is broken.

mod common;

use axum::http::StatusCode;
use burst_server::{db, ws::ServerEvent};
use uuid::Uuid;

fn auth(username: &str) -> String {
    format!("{username}@test.example")
}

async fn private_channel(pool: &sqlx::PgPool, name: &str, owner: Uuid) -> Uuid {
    let id = burst_core::id::new_id();
    db::channels::create(
        pool,
        &db::channels::CreateChannel {
            id,
            kind: "private".into(),
            name: Some(name.into()),
            slug: Some(name.into()),
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

fn ch(id: Uuid) -> String {
    burst_core::id::format_channel_id(id)
}

fn usr(id: Uuid) -> String {
    burst_core::id::format_user_id(id)
}

async fn is_member(pool: &sqlx::PgPool, channel: Uuid, user: Uuid) -> bool {
    db::channels::is_member(pool, channel, user).await.unwrap()
}

async fn add(pool: &sqlx::PgPool, channel: Uuid, user: Uuid, role: &str) {
    db::channels::add_member(pool, channel, user, role)
        .await
        .unwrap();
}

async fn post_message(app: &common::TestApp, channel: Uuid, as_user: &str, text: &str) -> String {
    let (status, body) = app
        .post(
            &format!("/api/channels/{}/messages", ch(channel)),
            &auth(as_user),
            serde_json::json!({ "content": text }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["id"].as_str().unwrap().to_string()
}

// ── Adding people ────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn a_member_adds_someone_to_a_private_channel(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await.id;
    let bob = common::seed_user(&pool, "bob").await.id;
    let room = private_channel(&pool, "secret", alice).await;
    let mut rx = app.state.broker.subscribe();

    let (status, body) = app
        .post(
            &format!("/api/channels/{}/members", ch(room)),
            &auth("alice"),
            serde_json::json!({ "userId": usr(bob) }),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    assert!(is_member(&pool, room, bob).await);

    let mut announced = false;
    while let Ok(ev) = rx.try_recv() {
        if let ServerEvent::ChannelJoined {
            user_id,
            channel_id,
            ..
        } = ev
        {
            announced |= user_id == usr(bob) && channel_id == ch(room);
        }
    }
    assert!(announced, "bob's open sockets must learn of the channel");
}

#[sqlx::test(migrations = "../../migrations")]
async fn only_a_member_can_add_someone(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await.id;
    let bob = common::seed_user(&pool, "bob").await.id;
    common::seed_user(&pool, "mallory").await;
    let room = private_channel(&pool, "secret", alice).await;

    let (status, _) = app
        .post(
            &format!("/api/channels/{}/members", ch(room)),
            &auth("mallory"),
            serde_json::json!({ "userId": usr(bob) }),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(!is_member(&pool, room, bob).await);
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_deactivated_user_cannot_be_added(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await.id;
    let gone = common::seed_user(&pool, "gone").await.id;
    sqlx::query("UPDATE users SET deactivated_at = now() WHERE id = $1")
        .bind(gone)
        .execute(&pool)
        .await
        .unwrap();
    let room = private_channel(&pool, "secret", alice).await;

    let (status, _) = app
        .post(
            &format!("/api/channels/{}/members", ch(room)),
            &auth("alice"),
            serde_json::json!({ "userId": usr(gone) }),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_private_channel_cannot_be_joined_uninvited(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await.id;
    let bob = common::seed_user(&pool, "bob").await.id;
    let room = private_channel(&pool, "secret", alice).await;

    for body in [
        serde_json::Value::Null,
        serde_json::json!({ "userId": usr(bob) }),
    ] {
        let (status, _) = app
            .post(
                &format!("/api/channels/{}/members", ch(room)),
                &auth("bob"),
                body,
            )
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }
    assert!(!is_member(&pool, room, bob).await);
}

#[sqlx::test(migrations = "../../migrations")]
async fn joining_a_public_channel_is_announced(pool: sqlx::PgPool) {
    // Without the announcement the joiner's live socket keeps ignoring the
    // channel, and its messages do not arrive until they reconnect.
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await.id;
    let bob = common::seed_user(&pool, "bob").await.id;
    let general = common::seed_channel(&pool, "general", alice).await.id;
    let mut rx = app.state.broker.subscribe();

    let (status, _) = app
        .post(
            &format!("/api/channels/{}/members", ch(general)),
            &auth("bob"),
            serde_json::Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let mut announced = false;
    while let Ok(ev) = rx.try_recv() {
        if let ServerEvent::ChannelJoined { user_id, .. } = ev {
            announced |= user_id == usr(bob);
        }
    }
    assert!(announced);
}

// ── Guests ───────────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn a_guest_cannot_create_a_channel_or_start_a_conversation(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let bob = common::seed_user(&pool, "bob").await.id;
    common::seed_user(&pool, "carol").await;
    common::seed_user_with_role(&pool, "gus", "guest").await;

    for (who, expected) in [
        ("carol", StatusCode::CREATED),
        ("gus", StatusCode::FORBIDDEN),
    ] {
        let (status, _) = app
            .post(
                "/api/channels",
                &auth(who),
                serde_json::json!({ "name": format!("by-{who}") }),
            )
            .await;
        assert_eq!(status, expected, "{who} creating a channel");

        let (status, _) = app
            .post(
                "/api/dms",
                &auth(who),
                serde_json::json!({ "userId": usr(bob) }),
            )
            .await;
        let ok = if who == "carol" {
            status.is_success()
        } else {
            status == StatusCode::FORBIDDEN
        };
        assert!(ok, "{who} starting a DM got {status}");
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_guest_cannot_find_or_enter_public_channels(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await.id;
    common::seed_user(&pool, "carol").await;
    let gus = common::seed_user_with_role(&pool, "gus", "guest").await.id;
    let general = common::seed_channel(&pool, "general", alice).await.id;

    // Browsing
    let (_, member_view) = app.get("/api/channels", &auth("carol")).await;
    assert_eq!(member_view["items"].as_array().unwrap().len(), 1, "control");
    let (status, guest_view) = app.get("/api/channels", &auth("gus")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        guest_view["items"].as_array().unwrap().is_empty(),
        "{guest_view}"
    );

    // Reading its details
    let (status, _) = app
        .get(&format!("/api/channels/{}", ch(general)), &auth("carol"))
        .await;
    assert_eq!(status, StatusCode::OK, "control");
    let (status, _) = app
        .get(&format!("/api/channels/{}", ch(general)), &auth("gus"))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Joining
    let (status, _) = app
        .post(
            &format!("/api/channels/{}/members", ch(general)),
            &auth("gus"),
            serde_json::Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(!is_member(&pool, general, gus).await);
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_guest_added_to_a_channel_reads_and_sends_there(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await.id;
    let gus = common::seed_user_with_role(&pool, "gus", "guest").await.id;
    let room = private_channel(&pool, "project", alice).await;

    let (status, _) = app
        .post(
            &format!("/api/channels/{}/members", ch(room)),
            &auth("alice"),
            serde_json::json!({ "userId": usr(gus) }),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    post_message(&app, room, "gus", "hello, thanks for the invite").await;
    let (status, messages) = app
        .get(
            &format!("/api/channels/{}/messages", ch(room)),
            &auth("gus"),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!messages["items"].as_array().unwrap().is_empty());
}

/// Everything reading and sending involves stays open to a guest. A
/// permission check that lands on a read path by mistake fails here, which is
/// how a pin check once ended up guarding `get_message`.
#[sqlx::test(migrations = "../../migrations")]
async fn a_guest_member_can_do_everything_reading_and_sending_involves(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await.id;
    let gus = common::seed_user_with_role(&pool, "gus", "guest").await.id;
    let room = private_channel(&pool, "project", alice).await;
    add(&pool, room, gus, "member").await;
    let msg = post_message(&app, room, "alice", "roadmap draft").await;
    let base = format!("/api/channels/{}", ch(room));

    for path in [
        base.clone(),
        format!("{base}/messages"),
        format!("{base}/messages/{msg}"),
        format!("{base}/messages/{msg}/replies"),
        format!("{base}/members"),
        format!("{base}/pins"),
        format!("/api/search/messages?q=roadmap&channelId={}", ch(room)),
    ] {
        let (status, body) = app.get(&path, &auth("gus")).await;
        assert_eq!(status, StatusCode::OK, "GET {path}: {body}");
    }

    let (status, _) = app
        .put(
            &format!("{base}/messages/{msg}/reactions/%F0%9F%91%8D"),
            &auth("gus"),
        )
        .await;
    assert!(status.is_success(), "reacting: {status}");

    let own = post_message(&app, room, "gus", "a typo").await;
    let (status, _) = app
        .patch(
            &format!("{base}/messages/{own}"),
            &auth("gus"),
            serde_json::json!({ "content": "fixed" }),
        )
        .await;
    assert!(status.is_success(), "editing their own message: {status}");
    let (status, _) = app
        .delete(&format!("{base}/messages/{own}"), &auth("gus"))
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "deleting their own message");
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_guest_member_cannot_edit_the_channel_pin_or_add_people(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await.id;
    let bob = common::seed_user(&pool, "bob").await.id;
    let gus = common::seed_user_with_role(&pool, "gus", "guest").await.id;
    let room = private_channel(&pool, "project", alice).await;
    add(&pool, room, gus, "member").await;
    let msg = post_message(&app, room, "alice", "the plan").await;

    for (who, allowed) in [("alice", true), ("gus", false)] {
        let (status, _) = app
            .patch(
                &format!("/api/channels/{}", ch(room)),
                &auth(who),
                serde_json::json!({ "topic": format!("set by {who}") }),
            )
            .await;
        assert_eq!(status.is_success(), allowed, "{who} editing: {status}");

        let (status, _) = app
            .put(
                &format!("/api/channels/{}/messages/{msg}/pin", ch(room)),
                &auth(who),
            )
            .await;
        assert_eq!(status.is_success(), allowed, "{who} pinning: {status}");
    }

    let (status, _) = app
        .post(
            &format!("/api/channels/{}/members", ch(room)),
            &auth("gus"),
            serde_json::json!({ "userId": usr(bob) }),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

// ── Moderation ───────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn a_member_cannot_delete_someone_elses_message(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await.id;
    let bob = common::seed_user(&pool, "bob").await.id;
    let general = common::seed_channel(&pool, "general", alice).await.id;
    add(&pool, general, bob, "member").await;
    let msg = post_message(&app, general, "alice", "keep this").await;

    let (status, _) = app
        .delete(
            &format!("/api/channels/{}/messages/{msg}", ch(general)),
            &auth("bob"),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_channel_moderator_deletes_someone_elses_message_and_it_is_recorded(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await.id;
    let mod_ = common::seed_user(&pool, "mod").await.id;
    let bob = common::seed_user(&pool, "bob").await.id;
    let general = common::seed_channel(&pool, "general", alice).await.id;
    add(&pool, general, mod_, "moderator").await;
    add(&pool, general, bob, "member").await;
    let msg = post_message(&app, general, "bob", "spam spam spam").await;

    let (status, _) = app
        .delete(
            &format!("/api/channels/{}/messages/{msg}", ch(general)),
            &auth("mod"),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let msg_id = burst_core::id::parse_prefixed_id(&msg, "msg_").unwrap();
    let row = db::messages::find_by_id(&pool, msg_id)
        .await
        .unwrap()
        .unwrap();
    assert!(row.deleted_at.is_some());

    let (entries,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM audit_log WHERE action = 'message.deleted_by_moderator' \
         AND target_id = $1 AND user_id = $2",
    )
    .bind(msg_id)
    .bind(mod_)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(entries, 1, "a moderator deletion must be audited");
}

#[sqlx::test(migrations = "../../migrations")]
async fn an_instance_moderator_moderates_channels_they_are_in_and_no_others(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await.id;
    let maud = common::seed_user_with_role(&pool, "maud", "moderator")
        .await
        .id;
    let joined = common::seed_channel(&pool, "joined", alice).await.id;
    let other = common::seed_channel(&pool, "other", alice).await.id;
    add(&pool, joined, maud, "member").await;
    let here = post_message(&app, joined, "alice", "a").await;
    let there = post_message(&app, other, "alice", "b").await;

    let (status, _) = app
        .delete(
            &format!("/api/channels/{}/messages/{here}", ch(joined)),
            &auth("maud"),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, _) = app
        .delete(
            &format!("/api/channels/{}/messages/{there}", ch(other)),
            &auth("maud"),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "not a member of #other");
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_moderator_archives_and_a_member_cannot(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await.id;
    let mod_ = common::seed_user(&pool, "mod").await.id;
    let bob = common::seed_user(&pool, "bob").await.id;
    let general = common::seed_channel(&pool, "general", alice).await.id;
    add(&pool, general, mod_, "moderator").await;
    add(&pool, general, bob, "member").await;

    let (status, _) = app
        .post(
            &format!("/api/channels/{}/archive", ch(general)),
            &auth("bob"),
            serde_json::Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = app
        .post(
            &format!("/api/channels/{}/archive", ch(general)),
            &auth("mod"),
            serde_json::Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
}

// ── Removing members ─────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn the_owner_removes_a_member_who_then_loses_access(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await.id;
    let bob = common::seed_user(&pool, "bob").await.id;
    let room = private_channel(&pool, "secret", alice).await;
    add(&pool, room, bob, "member").await;
    let mut rx = app.state.broker.subscribe();

    let (status, _) = app
        .delete(
            &format!("/api/channels/{}/members/{}", ch(room), usr(bob)),
            &auth("alice"),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(!is_member(&pool, room, bob).await);

    let mut announced = false;
    while let Ok(ev) = rx.try_recv() {
        if let ServerEvent::ChannelLeft {
            user_id,
            channel_id,
            ..
        } = ev
        {
            announced |= user_id == usr(bob) && channel_id == ch(room);
        }
    }
    assert!(
        announced,
        "bob's open sockets must stop forwarding the channel"
    );

    let (status, _) = app
        .get(
            &format!("/api/channels/{}/messages", ch(room)),
            &auth("bob"),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../../migrations")]
async fn removal_follows_rank(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await.id;
    let m1 = common::seed_user(&pool, "m1").await.id;
    let m2 = common::seed_user(&pool, "m2").await.id;
    let bob = common::seed_user(&pool, "bob").await.id;
    let carol = common::seed_user(&pool, "carol").await.id;
    common::seed_user_with_role(&pool, "root", "admin").await;
    let general = common::seed_channel(&pool, "general", alice).await.id;
    for (u, r) in [
        (m1, "moderator"),
        (m2, "moderator"),
        (bob, "member"),
        (carol, "member"),
    ] {
        add(&pool, general, u, r).await;
    }
    let remove = |who: &'static str, target: Uuid| {
        let app = &app;
        async move {
            app.delete(
                &format!("/api/channels/{}/members/{}", ch(general), usr(target)),
                &auth(who),
            )
            .await
            .0
        }
    };

    assert_eq!(
        remove("bob", carol).await,
        StatusCode::FORBIDDEN,
        "a member does not moderate"
    );
    assert_eq!(
        remove("m1", m2).await,
        StatusCode::FORBIDDEN,
        "moderators are peers"
    );
    assert_eq!(
        remove("m1", alice).await,
        StatusCode::FORBIDDEN,
        "nobody removes the owner"
    );
    assert_eq!(
        remove("root", alice).await,
        StatusCode::FORBIDDEN,
        "not even an admin"
    );
    assert_eq!(
        remove("m1", bob).await,
        StatusCode::NO_CONTENT,
        "a moderator removes a member"
    );
    assert_eq!(
        remove("alice", m2).await,
        StatusCode::NO_CONTENT,
        "the owner removes a moderator"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn removing_yourself_points_to_leaving(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await.id;
    let general = common::seed_channel(&pool, "general", alice).await.id;
    let (status, _) = app
        .delete(
            &format!("/api/channels/{}/members/{}", ch(general), usr(alice)),
            &auth("alice"),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../../migrations")]
async fn leaving_is_announced(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await.id;
    let bob = common::seed_user(&pool, "bob").await.id;
    let general = common::seed_channel(&pool, "general", alice).await.id;
    add(&pool, general, bob, "member").await;
    let mut rx = app.state.broker.subscribe();

    let (status, _) = app
        .delete(
            &format!("/api/channels/{}/members/me", ch(general)),
            &auth("bob"),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let mut announced = false;
    while let Ok(ev) = rx.try_recv() {
        announced |=
            matches!(ev, ServerEvent::ChannelLeft { ref user_id, .. } if *user_id == usr(bob));
    }
    assert!(announced);
}

// ── Appointing moderators ────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn the_owner_appoints_a_moderator_who_can_then_moderate(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await.id;
    let bob = common::seed_user(&pool, "bob").await.id;
    let carol = common::seed_user(&pool, "carol").await.id;
    let general = common::seed_channel(&pool, "general", alice).await.id;
    add(&pool, general, bob, "member").await;
    add(&pool, general, carol, "member").await;
    let msg = post_message(&app, general, "carol", "off topic").await;

    let (status, _) = app
        .delete(
            &format!("/api/channels/{}/messages/{msg}", ch(general)),
            &auth("bob"),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "control: bob cannot yet");

    let (status, body) = app
        .patch(
            &format!("/api/channels/{}/members/{}", ch(general), usr(bob)),
            &auth("alice"),
            serde_json::json!({ "role": "moderator" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["role"], "moderator");

    let (status, _) = app
        .delete(
            &format!("/api/channels/{}/messages/{msg}", ch(general)),
            &auth("bob"),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_moderator_cannot_appoint_and_ownership_cannot_be_granted(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await.id;
    let mod_ = common::seed_user(&pool, "mod").await.id;
    let bob = common::seed_user(&pool, "bob").await.id;
    let general = common::seed_channel(&pool, "general", alice).await.id;
    add(&pool, general, mod_, "moderator").await;
    add(&pool, general, bob, "member").await;
    let set = |who: &'static str, role: &'static str| {
        let app = &app;
        async move {
            app.patch(
                &format!("/api/channels/{}/members/{}", ch(general), usr(bob)),
                &auth(who),
                serde_json::json!({ "role": role }),
            )
            .await
            .0
        }
    };

    assert_eq!(set("mod", "moderator").await, StatusCode::FORBIDDEN);
    assert_eq!(set("alice", "owner").await, StatusCode::BAD_REQUEST);
    assert_eq!(set("alice", "moderator").await, StatusCode::OK, "control");
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_guest_cannot_be_made_a_moderator(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await.id;
    let gus = common::seed_user_with_role(&pool, "gus", "guest").await.id;
    let general = common::seed_channel(&pool, "general", alice).await.id;
    add(&pool, general, gus, "member").await;

    let (status, _) = app
        .patch(
            &format!("/api/channels/{}/members/{}", ch(general), usr(gus)),
            &auth("alice"),
            serde_json::json!({ "role": "moderator" }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
