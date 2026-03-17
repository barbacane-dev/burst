mod common;

use axum::http::StatusCode;
use burst_server::ws::ServerEvent;

// ── Helpers ──────────────────────────────────────────────────────────────────

fn auth(username: &str) -> String {
    format!("{username}@test.example")
}

fn pin_url(ch_id: &str, msg_id: &str) -> String {
    format!("/channels/{ch_id}/messages/{msg_id}/pin")
}

fn pins_url(ch_id: &str) -> String {
    format!("/channels/{ch_id}/pins")
}

async fn setup(pool: &sqlx::PgPool) -> (common::TestApp, String, String) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(pool, "alice").await;
    let ch = common::seed_channel(pool, "general", alice.id).await;
    let msg = common::seed_message(pool, ch.id, alice.id, "pin me!").await;

    let ch_id = format!("ch_{}", ch.id);
    let msg_id = format!("msg_{}", msg.id);
    (app, ch_id, msg_id)
}

// ── Pin message ──────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn pin_message_returns_no_content(pool: sqlx::PgPool) {
    let (app, ch_id, msg_id) = setup(&pool).await;

    let (status, _) = app.put(&pin_url(&ch_id, &msg_id), &auth("alice")).await;

    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[sqlx::test(migrations = "../../migrations")]
async fn pin_message_broadcasts_event(pool: sqlx::PgPool) {
    let (app, ch_id, msg_id) = setup(&pool).await;
    let mut rx = app.state.broker.subscribe();

    app.put(&pin_url(&ch_id, &msg_id), &auth("alice")).await;

    let mut found = false;
    while let Ok(ev) = rx.try_recv() {
        if matches!(ev, ServerEvent::MessagePinned { .. }) {
            found = true;
        }
    }
    assert!(found, "MessagePinned event must be broadcast");
}

#[sqlx::test(migrations = "../../migrations")]
async fn pin_same_message_twice_is_idempotent(pool: sqlx::PgPool) {
    let (app, ch_id, msg_id) = setup(&pool).await;
    let mut rx = app.state.broker.subscribe();

    app.put(&pin_url(&ch_id, &msg_id), &auth("alice")).await;
    app.put(&pin_url(&ch_id, &msg_id), &auth("alice")).await;

    let mut pin_count = 0u32;
    while let Ok(ev) = rx.try_recv() {
        if matches!(ev, ServerEvent::MessagePinned { .. }) {
            pin_count += 1;
        }
    }
    assert_eq!(
        pin_count, 1,
        "MessagePinned must broadcast only once — duplicate pin is a no-op"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn pin_by_non_member_is_forbidden(pool: sqlx::PgPool) {
    let (app, ch_id, msg_id) = setup(&pool).await;
    common::seed_user(&pool, "eve").await;

    let (status, _) = app.put(&pin_url(&ch_id, &msg_id), &auth("eve")).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../../migrations")]
async fn pin_nonexistent_message_returns_not_found(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await;
    let ch = common::seed_channel(&pool, "general", alice.id).await;

    let ch_id = format!("ch_{}", ch.id);
    let fake_msg_id = format!("msg_{}", burst_core::id::new_id());

    let (status, _) = app
        .put(&pin_url(&ch_id, &fake_msg_id), &auth("alice"))
        .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ── Unpin message ────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn unpin_message_returns_no_content(pool: sqlx::PgPool) {
    let (app, ch_id, msg_id) = setup(&pool).await;

    app.put(&pin_url(&ch_id, &msg_id), &auth("alice")).await;
    let (status, _) = app.delete(&pin_url(&ch_id, &msg_id), &auth("alice")).await;

    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[sqlx::test(migrations = "../../migrations")]
async fn unpin_broadcasts_event(pool: sqlx::PgPool) {
    let (app, ch_id, msg_id) = setup(&pool).await;

    app.put(&pin_url(&ch_id, &msg_id), &auth("alice")).await;

    let mut rx = app.state.broker.subscribe();
    app.delete(&pin_url(&ch_id, &msg_id), &auth("alice")).await;

    let mut found = false;
    while let Ok(ev) = rx.try_recv() {
        if matches!(ev, ServerEvent::MessageUnpinned { .. }) {
            found = true;
        }
    }
    assert!(found, "MessageUnpinned event must be broadcast");
}

#[sqlx::test(migrations = "../../migrations")]
async fn unpin_nonexistent_is_silent(pool: sqlx::PgPool) {
    let (app, ch_id, msg_id) = setup(&pool).await;
    let mut rx = app.state.broker.subscribe();

    // Never pinned — unpin should still be 204 but no event
    app.delete(&pin_url(&ch_id, &msg_id), &auth("alice")).await;

    let emitted = rx
        .try_recv()
        .ok()
        .map(|ev| matches!(ev, ServerEvent::MessageUnpinned { .. }))
        .unwrap_or(false);

    assert!(
        !emitted,
        "must NOT broadcast MessageUnpinned for a non-pinned message"
    );
}

// ── List pins ────────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn list_pins_returns_pinned_messages(pool: sqlx::PgPool) {
    let (app, ch_id, msg_id) = setup(&pool).await;

    app.put(&pin_url(&ch_id, &msg_id), &auth("alice")).await;

    let (status, body) = app.get(&pins_url(&ch_id), &auth("alice")).await;

    assert_eq!(status, StatusCode::OK);
    let items = body.as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["content"], "pin me!");
}

#[sqlx::test(migrations = "../../migrations")]
async fn list_pins_empty_channel(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await;
    let ch = common::seed_channel(&pool, "general", alice.id).await;
    let ch_id = format!("ch_{}", ch.id);

    let (status, body) = app.get(&pins_url(&ch_id), &auth("alice")).await;

    assert_eq!(status, StatusCode::OK);
    let items = body.as_array().unwrap();
    assert!(items.is_empty());
}

#[sqlx::test(migrations = "../../migrations")]
async fn list_pins_by_non_member_is_forbidden(pool: sqlx::PgPool) {
    let (app, ch_id, _) = setup(&pool).await;
    common::seed_user(&pool, "eve").await;

    let (status, _) = app.get(&pins_url(&ch_id), &auth("eve")).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../../migrations")]
async fn unpin_removes_from_pin_list(pool: sqlx::PgPool) {
    let (app, ch_id, msg_id) = setup(&pool).await;

    app.put(&pin_url(&ch_id, &msg_id), &auth("alice")).await;
    app.delete(&pin_url(&ch_id, &msg_id), &auth("alice")).await;

    let (_, body) = app.get(&pins_url(&ch_id), &auth("alice")).await;
    let items = body.as_array().unwrap();
    assert!(items.is_empty(), "pin list must be empty after unpin");
}

#[sqlx::test(migrations = "../../migrations")]
async fn pin_message_from_wrong_channel_returns_not_found(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await;
    let ch1 = common::seed_channel(&pool, "general", alice.id).await;
    let ch2 = common::seed_channel(&pool, "random", alice.id).await;
    let msg = common::seed_message(&pool, ch1.id, alice.id, "wrong channel").await;

    let ch2_id = format!("ch_{}", ch2.id);
    let msg_id = format!("msg_{}", msg.id);

    let (status, _) = app.put(&pin_url(&ch2_id, &msg_id), &auth("alice")).await;

    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "pinning a message from a different channel must fail"
    );
}
