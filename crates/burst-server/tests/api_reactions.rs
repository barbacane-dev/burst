mod common;

use axum::http::StatusCode;
use burst_server::ws::ServerEvent;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Set up a channel with Alice as owner, seed one message, return the
/// prefixed IDs ready for URL construction.
async fn setup(pool: &sqlx::PgPool) -> (common::TestApp, String, String) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(pool, "alice").await;
    let ch = common::seed_channel(pool, "general", alice.id).await;
    let msg = common::seed_message(pool, ch.id, alice.id, "hello").await;

    let ch_id = format!("ch_{}", ch.id);
    let msg_id = format!("msg_{}", msg.id);
    (app, ch_id, msg_id)
}

fn reaction_url(ch_id: &str, msg_id: &str, emoji: &str) -> String {
    format!(
        "/api/channels/{ch_id}/messages/{msg_id}/reactions/{}",
        urlencoding::encode(emoji)
    )
}

// ── Add reaction ──────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn add_reaction_returns_no_content(pool: sqlx::PgPool) {
    let (app, ch_id, msg_id) = setup(&pool).await;

    let (status, _) = app
        .put(&reaction_url(&ch_id, &msg_id, "👍"), "alice@test.example")
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

/// PUT on the same reaction twice must be idempotent and must only broadcast
/// a single `ReactionAdded` event.
#[sqlx::test(migrations = "../../migrations")]
async fn add_reaction_is_idempotent_and_broadcasts_once(pool: sqlx::PgPool) {
    let (app, ch_id, msg_id) = setup(&pool).await;
    let url = reaction_url(&ch_id, &msg_id, "❤️");

    let mut rx = app.state.broker.subscribe();

    app.put(&url, "alice@test.example").await;
    app.put(&url, "alice@test.example").await; // second PUT, must be a no-op

    let mut added_count = 0u32;
    while let Ok(ev) = rx.try_recv() {
        if matches!(ev, ServerEvent::ReactionAdded { .. }) {
            added_count += 1;
        }
    }

    assert_eq!(
        added_count, 1,
        "ReactionAdded must be broadcast exactly once — not on the duplicate PUT"
    );
}

/// After adding a reaction it must appear in the message's reaction list.
#[sqlx::test(migrations = "../../migrations")]
async fn reaction_appears_in_message_list(pool: sqlx::PgPool) {
    let (app, ch_id, msg_id) = setup(&pool).await;

    app.put(&reaction_url(&ch_id, &msg_id, "🚀"), "alice@test.example")
        .await;

    let (_, list) = app
        .get(
            &format!("/api/channels/{ch_id}/messages"),
            "alice@test.example",
        )
        .await;
    let msg = &list["items"].as_array().unwrap()[0];
    let reactions = msg["reactions"].as_array().unwrap();

    assert_eq!(reactions.len(), 1);
    assert_eq!(reactions[0]["emoji"], "🚀");
    assert_eq!(reactions[0]["count"], 1);
}

/// Non-members must receive 403.
#[sqlx::test(migrations = "../../migrations")]
async fn add_reaction_by_non_member_is_forbidden(pool: sqlx::PgPool) {
    let (app, ch_id, msg_id) = setup(&pool).await;
    common::seed_user(&pool, "eve").await;

    let (status, _) = app
        .put(&reaction_url(&ch_id, &msg_id, "👀"), "eve@test.example")
        .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

// ── Remove reaction ───────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn remove_reaction_returns_no_content(pool: sqlx::PgPool) {
    let (app, ch_id, msg_id) = setup(&pool).await;
    let url = reaction_url(&ch_id, &msg_id, "👎");

    app.put(&url, "alice@test.example").await;
    let (status, _) = app.delete(&url, "alice@test.example").await;

    assert_eq!(status, StatusCode::NO_CONTENT);
}

/// DELETE on a reaction that was never added must be a no-op (204) and must
/// not emit a `ReactionRemoved` event.
#[sqlx::test(migrations = "../../migrations")]
async fn remove_nonexistent_reaction_is_no_op_and_silent(pool: sqlx::PgPool) {
    let (app, ch_id, msg_id) = setup(&pool).await;

    let mut rx = app.state.broker.subscribe();

    let (status, _) = app
        .delete(&reaction_url(&ch_id, &msg_id, "😢"), "alice@test.example")
        .await;

    assert_eq!(status, StatusCode::NO_CONTENT, "DELETE must be idempotent");

    let emitted_remove = rx
        .try_recv()
        .ok()
        .map(|ev| matches!(ev, ServerEvent::ReactionRemoved { .. }))
        .unwrap_or(false);

    assert!(
        !emitted_remove,
        "ReactionRemoved must NOT be broadcast when the reaction didn't exist"
    );
}

/// After adding and removing a reaction it must no longer appear in the list.
#[sqlx::test(migrations = "../../migrations")]
async fn removed_reaction_disappears_from_message_list(pool: sqlx::PgPool) {
    let (app, ch_id, msg_id) = setup(&pool).await;
    let url = reaction_url(&ch_id, &msg_id, "🎉");

    app.put(&url, "alice@test.example").await;
    app.delete(&url, "alice@test.example").await;

    let (_, list) = app
        .get(
            &format!("/api/channels/{ch_id}/messages"),
            "alice@test.example",
        )
        .await;
    let reactions = list["items"].as_array().unwrap()[0]["reactions"]
        .as_array()
        .unwrap();

    assert!(reactions.is_empty(), "reaction must be gone after removal");
}
