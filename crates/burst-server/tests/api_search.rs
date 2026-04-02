mod common;

use axum::http::StatusCode;
use burst_server::db;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Returns `(app, ch_id_string)`.  Alice owns the channel and Bob is a member.
async fn setup(pool: &sqlx::PgPool) -> (common::TestApp, String) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(pool, "alice").await;
    let bob = common::seed_user(pool, "bob").await;
    let ch = common::seed_channel(pool, "general", alice.id).await;

    db::channels::add_member(pool, ch.id, bob.id, "member")
        .await
        .unwrap();

    let ch_id = format!("ch_{}", ch.id);
    (app, ch_id)
}

/// Returns `(app, ch_id, alice_id_string)` for tests that need the user id.
async fn setup_with_user(pool: &sqlx::PgPool) -> (common::TestApp, String, String) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(pool, "alice").await;
    let bob = common::seed_user(pool, "bob").await;
    let ch = common::seed_channel(pool, "general", alice.id).await;

    db::channels::add_member(pool, ch.id, bob.id, "member")
        .await
        .unwrap();

    let ch_id = format!("ch_{}", ch.id);
    let alice_id = format!("usr_{}", alice.id);
    (app, ch_id, alice_id)
}

// ── Search tests ─────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn search_returns_matching_messages(pool: sqlx::PgPool) {
    let (app, ch_id) = setup(&pool).await;

    app.post(
        &format!("/api/channels/{ch_id}/messages"),
        "alice@test.example",
        serde_json::json!({ "content": "the quick brown fox jumps" }),
    )
    .await;

    app.post(
        &format!("/api/channels/{ch_id}/messages"),
        "alice@test.example",
        serde_json::json!({ "content": "hello world" }),
    )
    .await;

    let (status, body) = app
        .get("/api/search/messages?q=fox", "alice@test.example")
        .await;

    assert_eq!(status, StatusCode::OK);
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert!(items[0]["content"].as_str().unwrap().contains("fox"));
    assert!(
        items[0]["headline"].as_str().unwrap().contains("<mark>"),
        "headline must contain highlight markers"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn search_respects_channel_membership(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await;
    let _charlie = common::seed_user(&pool, "charlie").await;
    let ch = common::seed_channel(&pool, "secret", alice.id).await;

    // Alice posts a message; Charlie is NOT a member.
    common::seed_message(&pool, ch.id, alice.id, "secret keyword").await;

    let (status, body) = app
        .get("/api/search/messages?q=keyword", "charlie@test.example")
        .await;

    assert_eq!(status, StatusCode::OK);
    let items = body["items"].as_array().unwrap();
    assert!(items.is_empty(), "non-member must not see results");
}

#[sqlx::test(migrations = "../../migrations")]
async fn search_empty_query_returns_bad_request(pool: sqlx::PgPool) {
    let (app, _) = setup(&pool).await;

    let (status, _) = app
        .get("/api/search/messages?q=", "alice@test.example")
        .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../../migrations")]
async fn search_filters_by_channel_id(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await;
    let ch1 = common::seed_channel(&pool, "room-a", alice.id).await;
    let ch2 = common::seed_channel(&pool, "room-b", alice.id).await;

    common::seed_message(&pool, ch1.id, alice.id, "matching keyword here").await;
    common::seed_message(&pool, ch2.id, alice.id, "matching keyword there").await;

    let ch1_id = format!("ch_{}", ch1.id);
    let (status, body) = app
        .get(
            &format!("/api/search/messages?q=keyword&channelId={ch1_id}"),
            "alice@test.example",
        )
        .await;

    assert_eq!(status, StatusCode::OK);
    let items = body["items"].as_array().unwrap();
    assert_eq!(
        items.len(),
        1,
        "must only return results from filtered channel"
    );
    assert_eq!(items[0]["channelId"], ch1_id);
}

#[sqlx::test(migrations = "../../migrations")]
async fn search_excludes_soft_deleted(pool: sqlx::PgPool) {
    let (app, ch_id) = setup(&pool).await;

    let (_, msg) = app
        .post(
            &format!("/api/channels/{ch_id}/messages"),
            "alice@test.example",
            serde_json::json!({ "content": "deletable keyword" }),
        )
        .await;
    let msg_id = msg["id"].as_str().unwrap();

    app.delete(
        &format!("/api/channels/{ch_id}/messages/{msg_id}"),
        "alice@test.example",
    )
    .await;

    let (status, body) = app
        .get("/api/search/messages?q=deletable", "alice@test.example")
        .await;

    assert_eq!(status, StatusCode::OK);
    let items = body["items"].as_array().unwrap();
    assert!(
        items.is_empty(),
        "soft-deleted messages must not appear in search"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn search_supports_cursor_pagination(pool: sqlx::PgPool) {
    let (app, ch_id) = setup(&pool).await;

    // Post several messages with the same keyword.
    for i in 1..=5 {
        app.post(
            &format!("/api/channels/{ch_id}/messages"),
            "alice@test.example",
            serde_json::json!({ "content": format!("paginate keyword {i}") }),
        )
        .await;
    }

    // Fetch first page with limit=2.
    let (status, page1) = app
        .get(
            "/api/search/messages?q=paginate&limit=2",
            "alice@test.example",
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let items1 = page1["items"].as_array().unwrap();
    assert_eq!(items1.len(), 2);
    let cursor = page1["cursor"]
        .as_str()
        .expect("cursor must be present for more results");

    // Fetch second page using cursor.
    let (status, page2) = app
        .get(
            &format!("/api/search/messages?q=paginate&limit=2&cursor={cursor}"),
            "alice@test.example",
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let items2 = page2["items"].as_array().unwrap();
    assert_eq!(items2.len(), 2);

    // Ensure no overlap between pages.
    let ids1: Vec<&str> = items1.iter().filter_map(|m| m["id"].as_str()).collect();
    let ids2: Vec<&str> = items2.iter().filter_map(|m| m["id"].as_str()).collect();
    for id in &ids1 {
        assert!(!ids2.contains(id), "pages must not overlap");
    }
}

// ── Additional search tests ─────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn search_query_too_long_returns_bad_request(pool: sqlx::PgPool) {
    let (app, _) = setup(&pool).await;

    let long_query: String = "a".repeat(201);
    let (status, _) = app
        .get(
            &format!("/api/search/messages?q={long_query}"),
            "alice@test.example",
        )
        .await;

    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "query exceeding 200 chars must return 400"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn search_no_q_param_returns_bad_request(pool: sqlx::PgPool) {
    let (app, _) = setup(&pool).await;

    let (status, _) = app.get("/api/search/messages", "alice@test.example").await;

    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "missing q param must return 400 (deserialization error)"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn search_returns_channel_and_user_ids(pool: sqlx::PgPool) {
    let (app, ch_id, alice_id) = setup_with_user(&pool).await;

    app.post(
        &format!("/api/channels/{ch_id}/messages"),
        "alice@test.example",
        serde_json::json!({ "content": "identifiable keyword" }),
    )
    .await;

    let (status, body) = app
        .get("/api/search/messages?q=identifiable", "alice@test.example")
        .await;

    assert_eq!(status, StatusCode::OK);
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);

    let result = &items[0];
    let channel_id = result["channelId"].as_str().unwrap();
    let user_id = result["userId"].as_str().unwrap();

    assert!(
        channel_id.starts_with("ch_"),
        "channelId must be prefixed with ch_, got: {channel_id}"
    );
    assert!(
        user_id.starts_with("usr_"),
        "userId must be prefixed with usr_, got: {user_id}"
    );
    assert_eq!(
        channel_id, ch_id,
        "channelId must match the posting channel"
    );
    assert_eq!(user_id, alice_id, "userId must match the posting user");
}

#[sqlx::test(migrations = "../../migrations")]
async fn search_headline_highlights_query_terms(pool: sqlx::PgPool) {
    let (app, ch_id) = setup(&pool).await;

    app.post(
        &format!("/api/channels/{ch_id}/messages"),
        "alice@test.example",
        serde_json::json!({ "content": "the highlight test should work" }),
    )
    .await;

    let (status, body) = app
        .get("/api/search/messages?q=highlight", "alice@test.example")
        .await;

    assert_eq!(status, StatusCode::OK);
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);

    let headline = items[0]["headline"].as_str().unwrap();
    assert!(
        headline.contains("<mark>"),
        "headline must contain opening <mark> tag, got: {headline}"
    );
    assert!(
        headline.contains("</mark>"),
        "headline must contain closing </mark> tag, got: {headline}"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn search_with_invalid_channel_id_returns_bad_request(pool: sqlx::PgPool) {
    let (app, _) = setup(&pool).await;

    let (status, _) = app
        .get(
            "/api/search/messages?q=test&channelId=not-a-valid-id",
            "alice@test.example",
        )
        .await;

    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "invalid channelId format must return 400"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn search_quoted_phrase_matching(pool: sqlx::PgPool) {
    let (app, ch_id) = setup(&pool).await;

    // Post a message with the exact phrase.
    app.post(
        &format!("/api/channels/{ch_id}/messages"),
        "alice@test.example",
        serde_json::json!({ "content": "the exact phrase appears here" }),
    )
    .await;

    // Post a message with the individual words but not the exact phrase.
    app.post(
        &format!("/api/channels/{ch_id}/messages"),
        "alice@test.example",
        serde_json::json!({ "content": "phrase is not exact in this message" }),
    )
    .await;

    // Search for the quoted exact phrase.
    let (status, body) = app
        .get(
            "/api/search/messages?q=%22exact+phrase%22",
            "alice@test.example",
        )
        .await;

    assert_eq!(status, StatusCode::OK);
    let items = body["items"].as_array().unwrap();
    assert_eq!(
        items.len(),
        1,
        "quoted phrase search must only return the exact match"
    );
    assert!(
        items[0]["content"]
            .as_str()
            .unwrap()
            .contains("exact phrase appears"),
        "result must contain the exact phrase"
    );
}
