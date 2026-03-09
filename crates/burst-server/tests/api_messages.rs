mod common;

use axum::http::StatusCode;
use burst_server::db;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Returns `(app, channel_id_string, alice_token, bob_token)`.
/// Alice owns the channel; Bob has joined it.
async fn setup(pool: &sqlx::PgPool) -> (common::TestApp, String, String, String) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(pool, "alice").await;
    let bob = common::seed_user(pool, "bob").await;
    let ch = common::seed_channel(pool, "general", alice.id).await;

    // Bob joins the channel.
    db::channels::add_member(pool, ch.id, bob.id, "member")
        .await
        .unwrap();

    let ch_id = format!("ch_{}", ch.id);
    let token_alice = app.token(alice.id, "member");
    let token_bob = app.token(bob.id, "member");
    (app, ch_id, token_alice, token_bob)
}

// ── Send message ──────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn send_message_returns_created(pool: sqlx::PgPool) {
    let (app, ch_id, token, _) = setup(&pool).await;

    let (status, body) = app
        .post(
            &format!("/channels/{ch_id}/messages"),
            &token,
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
    let (app, ch_id, token, _) = setup(&pool).await;

    for content in ["", "   ", "\t\n"] {
        let (status, _) = app
            .post(
                &format!("/channels/{ch_id}/messages"),
                &token,
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
    let (app, ch_id, token, _) = setup(&pool).await;

    // Post a root message.
    let (_, root) = app
        .post(
            &format!("/channels/{ch_id}/messages"),
            &token,
            serde_json::json!({ "content": "root" }),
        )
        .await;
    let root_id = root["id"].as_str().unwrap().to_owned();

    // Reply to root.
    let (_, reply1) = app
        .post(
            &format!("/channels/{ch_id}/messages"),
            &token,
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
            &format!("/channels/{ch_id}/messages"),
            &token,
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
    let (app, ch_id, token, _) = setup(&pool).await;

    let (_, root) = app
        .post(
            &format!("/channels/{ch_id}/messages"),
            &token,
            serde_json::json!({ "content": "root message" }),
        )
        .await;
    let root_id = root["id"].as_str().unwrap();

    app.post(
        &format!("/channels/{ch_id}/messages"),
        &token,
        serde_json::json!({ "content": "a reply", "threadId": root_id }),
    )
    .await;

    let (status, list) = app
        .get(&format!("/channels/{ch_id}/messages"), &token)
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
    let (app, ch_id, token, _) = setup(&pool).await;

    let (_, root) = app
        .post(
            &format!("/channels/{ch_id}/messages"),
            &token,
            serde_json::json!({ "content": "root" }),
        )
        .await;
    let root_id = root["id"].as_str().unwrap().to_owned();

    // Post a reply, then delete it.
    let (_, reply) = app
        .post(
            &format!("/channels/{ch_id}/messages"),
            &token,
            serde_json::json!({ "content": "to be deleted", "threadId": root_id }),
        )
        .await;
    let reply_id = reply["id"].as_str().unwrap();

    app.delete(&format!("/channels/{ch_id}/messages/{reply_id}"), &token)
        .await;

    // The root message's reply count must now be 0.
    let (_, list) = app
        .get(&format!("/channels/{ch_id}/messages"), &token)
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
    let (app, ch_id, token, _) = setup(&pool).await;

    let (_, msg) = app
        .post(
            &format!("/channels/{ch_id}/messages"),
            &token,
            serde_json::json!({ "content": "original" }),
        )
        .await;
    let msg_id = msg["id"].as_str().unwrap();

    let (status, edited) = app
        .patch(
            &format!("/channels/{ch_id}/messages/{msg_id}"),
            &token,
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
    let (app, ch_id, token_alice, token_bob) = setup(&pool).await;

    let (_, msg) = app
        .post(
            &format!("/channels/{ch_id}/messages"),
            &token_alice,
            serde_json::json!({ "content": "alice's message" }),
        )
        .await;
    let msg_id = msg["id"].as_str().unwrap();

    let (status, _) = app
        .patch(
            &format!("/channels/{ch_id}/messages/{msg_id}"),
            &token_bob,
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
    let (app, ch_id, token, _) = setup(&pool).await;

    let (_, msg) = app
        .post(
            &format!("/channels/{ch_id}/messages"),
            &token,
            serde_json::json!({ "content": "to be deleted" }),
        )
        .await;
    let msg_id = msg["id"].as_str().unwrap();

    let (status, _) = app
        .delete(&format!("/channels/{ch_id}/messages/{msg_id}"), &token)
        .await;

    assert_eq!(status, StatusCode::NO_CONTENT);
}

/// Soft delete must clear content and set `deletedAt`; the row is retained.
#[sqlx::test(migrations = "../../migrations")]
async fn soft_delete_clears_content_and_sets_deleted_at(pool: sqlx::PgPool) {
    let (app, ch_id, token, _) = setup(&pool).await;
    let alice = common::seed_user(&pool, "alice2").await;
    let ch = common::seed_channel(&pool, "room", alice.id).await;
    let token2 = app.token(alice.id, "member");

    let msg = common::seed_message(&pool, ch.id, alice.id, "will be deleted").await;

    let msg_id = format!("msg_{}", msg.id);
    let ch_id2 = format!("ch_{}", ch.id);
    app.delete(&format!("/channels/{ch_id2}/messages/{msg_id}"), &token2)
        .await;

    // The row must still be there in the DB with cleared content.
    let row = db::messages::find_by_id(&pool, msg.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.content, "", "content must be cleared on soft delete");
    assert!(row.deleted_at.is_some(), "deleted_at must be set");

    // Suppress unused-variable warning on ch_id.
    let _ = ch_id;
    let _ = token;
}

/// Another member cannot delete someone else's message.
#[sqlx::test(migrations = "../../migrations")]
async fn non_author_cannot_delete_message(pool: sqlx::PgPool) {
    let (app, ch_id, token_alice, token_bob) = setup(&pool).await;

    let (_, msg) = app
        .post(
            &format!("/channels/{ch_id}/messages"),
            &token_alice,
            serde_json::json!({ "content": "alice's message" }),
        )
        .await;
    let msg_id = msg["id"].as_str().unwrap();

    let (status, _) = app
        .delete(&format!("/channels/{ch_id}/messages/{msg_id}"), &token_bob)
        .await;

    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "deleting another user's message must return 404"
    );
}
