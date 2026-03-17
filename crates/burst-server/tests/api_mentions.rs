mod common;

use axum::http::StatusCode;
use burst_server::db;

// ── Helpers ──────────────────────────────────────────────────────────────────

fn auth(username: &str) -> String {
    format!("{username}@test.example")
}

async fn setup(pool: &sqlx::PgPool) -> (common::TestApp, String) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(pool, "alice").await;
    let ch = common::seed_channel(pool, "general", alice.id).await;

    let ch_id = format!("ch_{}", ch.id);
    (app, ch_id)
}

// ── @mentions in messages ────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn mention_is_persisted_when_message_sent(pool: sqlx::PgPool) {
    let (app, ch_id) = setup(&pool).await;
    let bob = common::seed_user(&pool, "bob").await;
    // Bob joins the channel so Alice can mention him
    db::channels::add_member(
        &pool,
        uuid::Uuid::parse_str(&ch_id[3..]).unwrap(),
        bob.id,
        "member",
    )
    .await
    .unwrap();

    let (status, body) = app
        .post(
            &format!("/channels/{ch_id}/messages"),
            &auth("alice"),
            serde_json::json!({ "content": "hey @bob check this" }),
        )
        .await;

    assert_eq!(status, StatusCode::CREATED);

    // Verify mention was stored in the database
    let msg_id_str = body["id"].as_str().unwrap();
    let msg_uuid = burst_core::id::parse_prefixed_id(msg_id_str, "msg_").unwrap();
    let mentions = db::mentions::list_for_messages(&pool, &[msg_uuid])
        .await
        .unwrap();

    assert_eq!(mentions.len(), 1, "should have exactly one mention");
    assert_eq!(mentions[0].1, bob.id, "mention should reference bob");
}

#[sqlx::test(migrations = "../../migrations")]
async fn multiple_mentions_in_one_message(pool: sqlx::PgPool) {
    let (app, ch_id) = setup(&pool).await;
    let bob = common::seed_user(&pool, "bob").await;
    let carol = common::seed_user(&pool, "carol").await;
    let ch_uuid = uuid::Uuid::parse_str(&ch_id[3..]).unwrap();
    db::channels::add_member(&pool, ch_uuid, bob.id, "member")
        .await
        .unwrap();
    db::channels::add_member(&pool, ch_uuid, carol.id, "member")
        .await
        .unwrap();

    let (status, body) = app
        .post(
            &format!("/channels/{ch_id}/messages"),
            &auth("alice"),
            serde_json::json!({ "content": "@bob and @carol please review" }),
        )
        .await;

    assert_eq!(status, StatusCode::CREATED);

    let msg_uuid = burst_core::id::parse_prefixed_id(body["id"].as_str().unwrap(), "msg_").unwrap();
    let mentions = db::mentions::list_for_messages(&pool, &[msg_uuid])
        .await
        .unwrap();

    assert_eq!(mentions.len(), 2, "should have two mentions");
    let mentioned_ids: Vec<uuid::Uuid> = mentions.iter().map(|m| m.1).collect();
    assert!(mentioned_ids.contains(&bob.id));
    assert!(mentioned_ids.contains(&carol.id));
}

#[sqlx::test(migrations = "../../migrations")]
async fn duplicate_mention_in_content_is_stored_once(pool: sqlx::PgPool) {
    let (app, ch_id) = setup(&pool).await;
    let bob = common::seed_user(&pool, "bob").await;
    let ch_uuid = uuid::Uuid::parse_str(&ch_id[3..]).unwrap();
    db::channels::add_member(&pool, ch_uuid, bob.id, "member")
        .await
        .unwrap();

    let (status, body) = app
        .post(
            &format!("/channels/{ch_id}/messages"),
            &auth("alice"),
            serde_json::json!({ "content": "@bob @bob @bob" }),
        )
        .await;

    assert_eq!(status, StatusCode::CREATED);

    let msg_uuid = burst_core::id::parse_prefixed_id(body["id"].as_str().unwrap(), "msg_").unwrap();
    let mentions = db::mentions::list_for_messages(&pool, &[msg_uuid])
        .await
        .unwrap();

    assert_eq!(mentions.len(), 1, "duplicate @bob must be stored only once");
}

#[sqlx::test(migrations = "../../migrations")]
async fn mention_nonexistent_user_is_silently_ignored(pool: sqlx::PgPool) {
    let (app, ch_id) = setup(&pool).await;

    let (status, body) = app
        .post(
            &format!("/channels/{ch_id}/messages"),
            &auth("alice"),
            serde_json::json!({ "content": "hey @nonexistent_user check this" }),
        )
        .await;

    assert_eq!(status, StatusCode::CREATED);

    let msg_uuid = burst_core::id::parse_prefixed_id(body["id"].as_str().unwrap(), "msg_").unwrap();
    let mentions = db::mentions::list_for_messages(&pool, &[msg_uuid])
        .await
        .unwrap();

    assert!(
        mentions.is_empty(),
        "mentioning a nonexistent user must not create a mention record"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn message_without_mentions_has_no_mention_records(pool: sqlx::PgPool) {
    let (app, ch_id) = setup(&pool).await;

    let (status, body) = app
        .post(
            &format!("/channels/{ch_id}/messages"),
            &auth("alice"),
            serde_json::json!({ "content": "just a plain message" }),
        )
        .await;

    assert_eq!(status, StatusCode::CREATED);

    let msg_uuid = burst_core::id::parse_prefixed_id(body["id"].as_str().unwrap(), "msg_").unwrap();
    let mentions = db::mentions::list_for_messages(&pool, &[msg_uuid])
        .await
        .unwrap();

    assert!(mentions.is_empty());
}

#[sqlx::test(migrations = "../../migrations")]
async fn mention_at_start_of_message(pool: sqlx::PgPool) {
    let (app, ch_id) = setup(&pool).await;
    let bob = common::seed_user(&pool, "bob").await;
    let ch_uuid = uuid::Uuid::parse_str(&ch_id[3..]).unwrap();
    db::channels::add_member(&pool, ch_uuid, bob.id, "member")
        .await
        .unwrap();

    let (status, body) = app
        .post(
            &format!("/channels/{ch_id}/messages"),
            &auth("alice"),
            serde_json::json!({ "content": "@bob hello!" }),
        )
        .await;

    assert_eq!(status, StatusCode::CREATED);

    let msg_uuid = burst_core::id::parse_prefixed_id(body["id"].as_str().unwrap(), "msg_").unwrap();
    let mentions = db::mentions::list_for_messages(&pool, &[msg_uuid])
        .await
        .unwrap();

    assert_eq!(mentions.len(), 1);
}

#[sqlx::test(migrations = "../../migrations")]
async fn email_like_text_is_not_a_mention(pool: sqlx::PgPool) {
    let (app, ch_id) = setup(&pool).await;
    common::seed_user(&pool, "bob").await;

    let (status, body) = app
        .post(
            &format!("/channels/{ch_id}/messages"),
            &auth("alice"),
            serde_json::json!({ "content": "send it to alice@bob.com" }),
        )
        .await;

    assert_eq!(status, StatusCode::CREATED);

    let msg_uuid = burst_core::id::parse_prefixed_id(body["id"].as_str().unwrap(), "msg_").unwrap();
    let mentions = db::mentions::list_for_messages(&pool, &[msg_uuid])
        .await
        .unwrap();

    // "alice@bob.com" — the @ is preceded by an alphanumeric char, so
    // parse_mentions should NOT treat "bob" as a mention.
    assert!(
        mentions.is_empty(),
        "email-like patterns must not be treated as mentions"
    );
}
