//! Custom status: set, replace, clear, expire, and announce.

mod common;

use std::time::Duration;

use axum::http::StatusCode;
use chrono::Utc;
use serde_json::{Value, json};
use uuid::Uuid;

const ARRIVES: Duration = Duration::from_secs(3);

fn auth(username: &str) -> String {
    format!("{username}@test.example")
}

fn in_one_hour() -> String {
    (Utc::now() + chrono::Duration::hours(1)).to_rfc3339()
}

async fn set(app: &common::TestApp, as_user: &str, body: Value) -> (StatusCode, Value) {
    app.put_json("/api/users/me/status", &auth(as_user), body)
        .await
}

async fn profile_seen_by(app: &common::TestApp, viewer: &str, user: Uuid) -> Value {
    let (status, body) = app
        .get(
            &format!("/api/users/{}", burst_core::id::format_user_id(user)),
            &auth(viewer),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_status_is_set_and_seen_by_others(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await.id;
    common::seed_user(&pool, "bob").await;
    let until = in_one_hour();

    let (status, body) = set(
        &app,
        "alice",
        json!({ "text": " In a meeting ", "emoji": "📅", "expiresAt": until }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["statusText"], "In a meeting");
    assert_eq!(body["statusEmoji"], "📅");

    let seen = profile_seen_by(&app, "bob", alice).await;
    assert_eq!(seen["statusText"], "In a meeting");
    assert_eq!(seen["statusEmoji"], "📅");
    let expires: chrono::DateTime<Utc> = seen["statusExpiresAt"].as_str().unwrap().parse().unwrap();
    assert_eq!(
        expires.timestamp(),
        until.parse::<chrono::DateTime<Utc>>().unwrap().timestamp()
    );

    let (_, list) = app.get("/api/users", &auth("bob")).await;
    let in_list = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["username"] == "alice")
        .unwrap();
    assert_eq!(in_list["statusEmoji"], "📅");
}

#[sqlx::test(migrations = "../../migrations")]
async fn setting_a_status_replaces_the_whole_previous_one(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    common::seed_user(&pool, "alice").await;

    set(
        &app,
        "alice",
        json!({ "text": "Lunch", "emoji": "🍕", "expiresAt": in_one_hour() }),
    )
    .await;
    let (status, body) = set(&app, "alice", json!({ "text": "Focusing" })).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["statusText"], "Focusing");
    assert!(body.get("statusEmoji").is_none(), "{body}");
    assert!(body.get("statusExpiresAt").is_none(), "{body}");
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_status_is_cleared(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await.id;
    common::seed_user(&pool, "bob").await;
    set(&app, "alice", json!({ "text": "Away", "emoji": "🌴" })).await;
    assert_eq!(
        profile_seen_by(&app, "bob", alice).await["statusText"],
        "Away",
        "control"
    );

    let (status, body) = app.delete("/api/users/me/status", &auth("alice")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let seen = profile_seen_by(&app, "bob", alice).await;
    for field in ["statusText", "statusEmoji", "statusExpiresAt"] {
        assert!(
            seen.get(field).is_none_or(Value::is_null),
            "{field} in {seen}"
        );
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn an_expired_status_is_reported_as_none(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await.id;
    common::seed_user(&pool, "bob").await;
    set(
        &app,
        "alice",
        json!({ "text": "Back soon", "emoji": "⏳", "expiresAt": in_one_hour() }),
    )
    .await;
    assert_eq!(
        profile_seen_by(&app, "bob", alice).await["statusEmoji"],
        "⏳",
        "control"
    );

    sqlx::query("UPDATE users SET status_expires_at = now() - interval '1 second' WHERE id = $1")
        .bind(alice)
        .execute(&pool)
        .await
        .unwrap();

    let seen = profile_seen_by(&app, "bob", alice).await;
    for field in ["statusText", "statusEmoji", "statusExpiresAt"] {
        assert!(
            seen.get(field).is_none_or(Value::is_null),
            "{field} in {seen}"
        );
    }
    let (_, me) = app.get("/api/users/me", &auth("alice")).await;
    assert!(me.get("statusText").is_none_or(Value::is_null), "{me}");
}

#[sqlx::test(migrations = "../../migrations")]
async fn an_invalid_status_is_refused_and_the_old_one_kept(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await.id;
    common::seed_user(&pool, "bob").await;
    set(&app, "alice", json!({ "text": "Kept" })).await;

    let past = (Utc::now() - chrono::Duration::minutes(1)).to_rfc3339();
    for body in [
        json!({}),
        json!({ "text": "   " }),
        json!({ "text": "x".repeat(101) }),
        json!({ "emoji": "busy" }),
        json!({ "text": "x", "expiresAt": past }),
        json!({ "text": "x", "expiresAt": "tomorrow" }),
    ] {
        let (status, response) = set(&app, "alice", body.clone()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body} gave {response}");
    }
    assert_eq!(
        profile_seen_by(&app, "bob", alice).await["statusText"],
        "Kept"
    );
}

// ── Live ────────────────────────────────────────────────────────────────────

fn is_status_of(user: Uuid) -> impl Fn(&Value) -> bool {
    let id = burst_core::id::format_user_id(user);
    move |e| e["type"] == "user.status_changed" && e["userId"] == id
}

/// A socket for `user`, known to be subscribed. The user gets a channel of
/// their own and posts there until their socket delivers it: a socket sends
/// its own presence event before it subscribes, so that is no signal.
async fn subscribed_socket(
    app: &common::TestApp,
    pool: &sqlx::PgPool,
    user: Uuid,
) -> common::Socket {
    let solo = common::seed_channel(pool, "solo", user).await.id;
    let addr = app.serve().await;
    let row = burst_server::db::users::find_by_id(pool, user)
        .await
        .unwrap()
        .unwrap();
    let mut socket = common::connect(addr, &auth(&row.username)).await;
    for attempt in 0..20 {
        let text = format!("ready {attempt}");
        app.post(
            &format!(
                "/api/channels/{}/messages",
                burst_core::id::format_channel_id(solo)
            ),
            &auth(&row.username),
            json!({ "content": text }),
        )
        .await;
        if common::wait_for(&mut socket, Duration::from_millis(200), |e| {
            e["type"] == "message.created" && e["message"]["content"] == text.as_str()
        })
        .await
        .is_some()
        {
            return socket;
        }
    }
    panic!("the socket never became ready");
}

/// Bob shares no channel with Alice: a status is announced to everyone.
#[sqlx::test(migrations = "../../migrations")]
async fn setting_and_clearing_are_announced_to_everyone(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await.id;
    let bob = common::seed_user(&pool, "bob").await.id;
    let mut bob = subscribed_socket(&app, &pool, bob).await;

    set(&app, "alice", json!({ "text": "Offsite", "emoji": "🚆" })).await;
    let event = common::wait_for(&mut bob, ARRIVES, is_status_of(alice))
        .await
        .expect("set is announced");
    assert_eq!(event["text"], "Offsite");
    assert_eq!(event["emoji"], "🚆");

    app.delete("/api/users/me/status", &auth("alice")).await;
    let event = common::wait_for(&mut bob, ARRIVES, is_status_of(alice))
        .await
        .expect("clear is announced");
    assert!(
        event.get("text").is_none() && event.get("emoji").is_none(),
        "{event}"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn the_profile_status_text_field_is_announced_too(pool: sqlx::PgPool) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(&pool, "alice").await.id;
    let bob = common::seed_user(&pool, "bob").await.id;
    let mut bob = subscribed_socket(&app, &pool, bob).await;

    app.patch(
        "/api/users/me",
        &auth("alice"),
        json!({ "statusText": "On leave" }),
    )
    .await;
    let event = common::wait_for(&mut bob, ARRIVES, is_status_of(alice))
        .await
        .expect("announced");
    assert_eq!(event["text"], "On leave");
}
