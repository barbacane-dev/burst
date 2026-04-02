mod common;

use axum::http::StatusCode;
use burst_server::db;
use burst_server::ws::ServerEvent;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Returns `(app, channel_id_string, alice_ext_id, bob_ext_id)`.
/// Alice owns the channel; Bob has joined it.
async fn setup(pool: &sqlx::PgPool) -> (common::TestApp, String) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(pool, "alice").await;
    let bob = common::seed_user(pool, "bob").await;
    let ch = common::seed_channel(pool, "general", alice.id).await;

    // Bob joins the channel.
    db::channels::add_member(pool, ch.id, bob.id, "member")
        .await
        .unwrap();

    let ch_id = format!("ch_{}", ch.id);
    (app, ch_id)
}

// ── Send message ──────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn send_message_returns_created(pool: sqlx::PgPool) {
    let (app, ch_id) = setup(&pool).await;

    let (status, body) = app
        .post(
            &format!("/api/channels/{ch_id}/messages"),
            "alice@test.example",
            serde_json::json!({ "content": "hello world" }),
        )
        .await;

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["content"], "hello world");
    assert!(body["id"].as_str().unwrap().starts_with("msg_"));
    assert!(body["deletedAt"].is_null());
    assert!(body["editedAt"].is_null());
}

#[sqlx::test(migrations = "../../migrations")]
async fn send_message_empty_content_is_bad_request(pool: sqlx::PgPool) {
    let (app, ch_id) = setup(&pool).await;

    for content in ["", "   ", "\t\n"] {
        let (status, _) = app
            .post(
                &format!("/api/channels/{ch_id}/messages"),
                "alice@test.example",
                serde_json::json!({ "content": content }),
            )
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "content={content:?}");
    }
}

// ── Flat threading ────────────────────────────────────────────────────────────

/// When replying to a reply, the stored thread_id must be the root message's
/// ID (not the intermediate reply) — flat threading is enforced in the API.
#[sqlx::test(migrations = "../../migrations")]
async fn reply_to_reply_stores_root_as_thread_id(pool: sqlx::PgPool) {
    let (app, ch_id) = setup(&pool).await;

    // Post a root message.
    let (_, root) = app
        .post(
            &format!("/api/channels/{ch_id}/messages"),
            "alice@test.example",
            serde_json::json!({ "content": "root" }),
        )
        .await;
    let root_id = root["id"].as_str().unwrap().to_owned();

    // Reply to root.
    let (_, reply1) = app
        .post(
            &format!("/api/channels/{ch_id}/messages"),
            "alice@test.example",
            serde_json::json!({ "content": "first reply", "threadId": root_id }),
        )
        .await;
    let reply1_id = reply1["id"].as_str().unwrap().to_owned();
    assert_eq!(
        reply1["threadId"], root_id,
        "first reply must point to root"
    );

    // Reply to the reply — thread_id must still be root, not reply1.
    let (_, reply2) = app
        .post(
            &format!("/api/channels/{ch_id}/messages"),
            "alice@test.example",
            serde_json::json!({ "content": "nested reply", "threadId": reply1_id }),
        )
        .await;

    assert_eq!(
        reply2["threadId"], root_id,
        "reply to a reply must store root ID to enforce flat threading"
    );
}

/// Thread replies must not appear in the main channel feed.
#[sqlx::test(migrations = "../../migrations")]
async fn thread_replies_excluded_from_channel_feed(pool: sqlx::PgPool) {
    let (app, ch_id) = setup(&pool).await;

    let (_, root) = app
        .post(
            &format!("/api/channels/{ch_id}/messages"),
            "alice@test.example",
            serde_json::json!({ "content": "root message" }),
        )
        .await;
    let root_id = root["id"].as_str().unwrap();

    app.post(
        &format!("/api/channels/{ch_id}/messages"),
        "alice@test.example",
        serde_json::json!({ "content": "a reply", "threadId": root_id }),
    )
    .await;

    let (status, list) = app
        .get(
            &format!("/api/channels/{ch_id}/messages"),
            "alice@test.example",
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let ids: Vec<&str> = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|m| m["id"].as_str())
        .collect();

    assert!(ids.contains(&root_id), "root must be in the feed");
    assert_eq!(ids.len(), 1, "reply must not appear in main feed");
}

/// The reply count reported in the feed must exclude soft-deleted replies.
#[sqlx::test(migrations = "../../migrations")]
async fn deleted_replies_excluded_from_reply_count(pool: sqlx::PgPool) {
    let (app, ch_id) = setup(&pool).await;

    let (_, root) = app
        .post(
            &format!("/api/channels/{ch_id}/messages"),
            "alice@test.example",
            serde_json::json!({ "content": "root" }),
        )
        .await;
    let root_id = root["id"].as_str().unwrap().to_owned();

    // Post a reply, then delete it.
    let (_, reply) = app
        .post(
            &format!("/api/channels/{ch_id}/messages"),
            "alice@test.example",
            serde_json::json!({ "content": "to be deleted", "threadId": root_id }),
        )
        .await;
    let reply_id = reply["id"].as_str().unwrap();

    app.delete(
        &format!("/api/channels/{ch_id}/messages/{reply_id}"),
        "alice@test.example",
    )
    .await;

    // The root message's reply count must now be 0.
    let (_, list) = app
        .get(
            &format!("/api/channels/{ch_id}/messages"),
            "alice@test.example",
        )
        .await;
    let root_msg = &list["items"].as_array().unwrap()[0];
    assert_eq!(
        root_msg["replyCount"], 0,
        "soft-deleted reply must not count toward replyCount"
    );
}

// ── Edit message ──────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn author_can_edit_own_message(pool: sqlx::PgPool) {
    let (app, ch_id) = setup(&pool).await;

    let (_, msg) = app
        .post(
            &format!("/api/channels/{ch_id}/messages"),
            "alice@test.example",
            serde_json::json!({ "content": "original" }),
        )
        .await;
    let msg_id = msg["id"].as_str().unwrap();

    let (status, edited) = app
        .patch(
            &format!("/api/channels/{ch_id}/messages/{msg_id}"),
            "alice@test.example",
            serde_json::json!({ "content": "updated" }),
        )
        .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(edited["content"], "updated");
    assert!(
        !edited["editedAt"].is_null(),
        "editedAt must be set after edit"
    );
}

/// Another member cannot edit someone else's message — returns 404 (not 403)
/// so the existence of the message is not confirmed to the requester.
#[sqlx::test(migrations = "../../migrations")]
async fn non_author_cannot_edit_message(pool: sqlx::PgPool) {
    let (app, ch_id) = setup(&pool).await;

    let (_, msg) = app
        .post(
            &format!("/api/channels/{ch_id}/messages"),
            "alice@test.example",
            serde_json::json!({ "content": "alice's message" }),
        )
        .await;
    let msg_id = msg["id"].as_str().unwrap();

    let (status, _) = app
        .patch(
            &format!("/api/channels/{ch_id}/messages/{msg_id}"),
            "bob@test.example",
            serde_json::json!({ "content": "bob's edit" }),
        )
        .await;

    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "editing another user's message must return 404"
    );
}

// ── Delete message ────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn author_can_delete_own_message(pool: sqlx::PgPool) {
    let (app, ch_id) = setup(&pool).await;

    let (_, msg) = app
        .post(
            &format!("/api/channels/{ch_id}/messages"),
            "alice@test.example",
            serde_json::json!({ "content": "to be deleted" }),
        )
        .await;
    let msg_id = msg["id"].as_str().unwrap();

    let (status, _) = app
        .delete(
            &format!("/api/channels/{ch_id}/messages/{msg_id}"),
            "alice@test.example",
        )
        .await;

    assert_eq!(status, StatusCode::NO_CONTENT);
}

/// Soft delete must clear content and set `deletedAt`; the row is retained.
#[sqlx::test(migrations = "../../migrations")]
async fn soft_delete_clears_content_and_sets_deleted_at(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice2").await;
    let ch = common::seed_channel(&pool, "room", alice.id).await;

    let msg = common::seed_message(&pool, ch.id, alice.id, "will be deleted").await;

    let msg_id = format!("msg_{}", msg.id);
    let ch_id = format!("ch_{}", ch.id);
    app.delete(
        &format!("/api/channels/{ch_id}/messages/{msg_id}"),
        "alice2@test.example",
    )
    .await;

    // The row must still be there in the DB with cleared content.
    let row = db::messages::find_by_id(&pool, msg.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.content, "", "content must be cleared on soft delete");
    assert!(row.deleted_at.is_some(), "deleted_at must be set");
}

/// Another member cannot delete someone else's message.
#[sqlx::test(migrations = "../../migrations")]
async fn non_author_cannot_delete_message(pool: sqlx::PgPool) {
    let (app, ch_id) = setup(&pool).await;

    let (_, msg) = app
        .post(
            &format!("/api/channels/{ch_id}/messages"),
            "alice@test.example",
            serde_json::json!({ "content": "alice's message" }),
        )
        .await;
    let msg_id = msg["id"].as_str().unwrap();

    let (status, _) = app
        .delete(
            &format!("/api/channels/{ch_id}/messages/{msg_id}"),
            "bob@test.example",
        )
        .await;

    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "deleting another user's message must return 404"
    );
}

// ── Broker event tests ───────────────────────────────────────────────────────

/// Sending a message must broadcast a MessageCreated event on the broker.
#[sqlx::test(migrations = "../../migrations")]
async fn send_message_broadcasts_message_created(pool: sqlx::PgPool) {
    let (app, ch_id) = setup(&pool).await;
    let mut rx = app.state.broker.subscribe();

    let (status, msg) = app
        .post(
            &format!("/api/channels/{ch_id}/messages"),
            "alice@test.example",
            serde_json::json!({ "content": "hello" }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let msg_id = msg["id"].as_str().unwrap();

    let mut found = false;
    while let Ok(ev) = rx.try_recv() {
        if let ServerEvent::MessageCreated {
            message,
            channel_id,
            ..
        } = &ev
        {
            assert_eq!(message.id, msg_id);
            assert_eq!(channel_id, &ch_id);
            found = true;
        }
    }
    assert!(found, "MessageCreated event must be broadcast");
}

/// Editing a message must broadcast a MessageUpdated event.
#[sqlx::test(migrations = "../../migrations")]
async fn edit_message_broadcasts_message_updated(pool: sqlx::PgPool) {
    let (app, ch_id) = setup(&pool).await;

    let (_, msg) = app
        .post(
            &format!("/api/channels/{ch_id}/messages"),
            "alice@test.example",
            serde_json::json!({ "content": "original" }),
        )
        .await;
    let msg_id = msg["id"].as_str().unwrap();

    let mut rx = app.state.broker.subscribe();

    let (status, _) = app
        .patch(
            &format!("/api/channels/{ch_id}/messages/{msg_id}"),
            "alice@test.example",
            serde_json::json!({ "content": "updated" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let mut found = false;
    while let Ok(ev) = rx.try_recv() {
        if let ServerEvent::MessageUpdated { message, .. } = &ev {
            assert_eq!(message.content, "updated");
            found = true;
        }
    }
    assert!(found, "MessageUpdated event must be broadcast");
}

/// Deleting a message must broadcast a MessageDeleted event.
#[sqlx::test(migrations = "../../migrations")]
async fn delete_message_broadcasts_message_deleted(pool: sqlx::PgPool) {
    let (app, ch_id) = setup(&pool).await;

    let (_, msg) = app
        .post(
            &format!("/api/channels/{ch_id}/messages"),
            "alice@test.example",
            serde_json::json!({ "content": "to delete" }),
        )
        .await;
    let msg_id = msg["id"].as_str().unwrap().to_owned();

    let mut rx = app.state.broker.subscribe();

    let (status, _) = app
        .delete(
            &format!("/api/channels/{ch_id}/messages/{msg_id}"),
            "alice@test.example",
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let mut found = false;
    while let Ok(ev) = rx.try_recv() {
        if let ServerEvent::MessageDeleted { message_id, .. } = &ev {
            assert_eq!(message_id, &msg_id);
            found = true;
        }
    }
    assert!(found, "MessageDeleted event must be broadcast");
}

/// Messages sent by a peer must increase the channel's unread count.
#[sqlx::test(migrations = "../../migrations")]
async fn peer_message_increments_unread_count(pool: sqlx::PgPool) {
    let (app, ch_id) = setup(&pool).await;

    // Bob sends a message.
    let (status, _) = app
        .post(
            &format!("/api/channels/{ch_id}/messages"),
            "bob@test.example",
            serde_json::json!({ "content": "hey alice" }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);

    // Alice's joined channels should show unread count > 0.
    let (status, list) = app
        .get("/api/channels?joined=true", "alice@test.example")
        .await;
    assert_eq!(status, StatusCode::OK);

    let ch = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"].as_str() == Some(&ch_id))
        .expect("channel must be in alice's list");
    assert!(
        ch["unreadCount"].as_i64().unwrap() > 0,
        "unread count must increase after peer message"
    );
}

/// Marking a channel as read must reset the unread count to zero.
#[sqlx::test(migrations = "../../migrations")]
async fn mark_read_resets_unread_count(pool: sqlx::PgPool) {
    let (app, ch_id) = setup(&pool).await;

    // Bob sends a message so Alice has unreads.
    app.post(
        &format!("/api/channels/{ch_id}/messages"),
        "bob@test.example",
        serde_json::json!({ "content": "unread msg" }),
    )
    .await;

    // Alice marks the channel as read.
    let (status, _) = app
        .patch(
            &format!("/api/channels/{ch_id}/members/me/last-read"),
            "alice@test.example",
            serde_json::json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // Alice's unread count should now be 0.
    let (_, list) = app
        .get("/api/channels?joined=true", "alice@test.example")
        .await;
    let ch = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"].as_str() == Some(&ch_id))
        .expect("channel must be in alice's list");
    assert_eq!(
        ch["unreadCount"].as_i64().unwrap(),
        0,
        "unread count must be 0 after marking read"
    );
}

/// Listing messages in a channel returns them in reverse chronological order.
#[sqlx::test(migrations = "../../migrations")]
async fn list_messages_returns_newest_first(pool: sqlx::PgPool) {
    let (app, ch_id) = setup(&pool).await;

    for i in 1..=3 {
        app.post(
            &format!("/api/channels/{ch_id}/messages"),
            "alice@test.example",
            serde_json::json!({ "content": format!("msg {i}") }),
        )
        .await;
    }

    let (status, list) = app
        .get(
            &format!("/api/channels/{ch_id}/messages"),
            "alice@test.example",
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let messages = list["items"].as_array().unwrap();
    assert_eq!(messages.len(), 3);
    assert_eq!(messages[0]["content"], "msg 3", "newest must be first");
    assert_eq!(messages[2]["content"], "msg 1", "oldest must be last");
}

/// Non-member must not be able to list messages.
#[sqlx::test(migrations = "../../migrations")]
async fn non_member_cannot_list_messages(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await;
    let _charlie = common::seed_user(&pool, "charlie").await;
    let ch = common::seed_channel(&pool, "private", alice.id).await;
    let ch_id = format!("ch_{}", ch.id);

    let (status, _) = app
        .get(
            &format!("/api/channels/{ch_id}/messages"),
            "charlie@test.example",
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

/// Non-member must not be able to send a message.
#[sqlx::test(migrations = "../../migrations")]
async fn non_member_cannot_send_message(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await;
    let _charlie = common::seed_user(&pool, "charlie").await;
    let ch = common::seed_channel(&pool, "private", alice.id).await;
    let ch_id = format!("ch_{}", ch.id);

    let (status, _) = app
        .post(
            &format!("/api/channels/{ch_id}/messages"),
            "charlie@test.example",
            serde_json::json!({ "content": "sneaky" }),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
