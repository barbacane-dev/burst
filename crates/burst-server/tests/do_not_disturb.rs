//! Do not disturb: settings, and notifications held back while quiet.

mod common;

use axum::http::StatusCode;
use burst_server::{db, ws::ServerEvent};
use chrono::{Duration, Utc};
use serde_json::{Value, json};
use uuid::Uuid;

const DND: &str = "/api/users/me/do-not-disturb";
const ALL_DAYS: [&str; 7] = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];

fn auth(username: &str) -> String {
    format!("{username}@test.example")
}

struct Setup {
    app: common::TestApp,
    pool: sqlx::PgPool,
    channel: String,
    bob: Uuid,
    carol: Uuid,
}

/// `general`, owned by alice, with bob and carol as members.
async fn setup(pool: &sqlx::PgPool) -> Setup {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(pool, "alice").await.id;
    let bob = common::seed_user(pool, "bob").await.id;
    let carol = common::seed_user(pool, "carol").await.id;
    let ch = common::seed_channel(pool, "general", alice).await.id;
    for user in [bob, carol] {
        db::channels::add_member(pool, ch, user, "member")
            .await
            .unwrap();
    }
    Setup {
        app,
        pool: pool.clone(),
        channel: burst_core::id::format_channel_id(ch),
        bob,
        carol,
    }
}

impl Setup {
    async fn set(&self, user: &str, body: Value) -> (StatusCode, Value) {
        self.app.put_json(DND, &auth(user), body).await
    }

    /// Alice posts `content`; returns who was notified.
    async fn post(&self, content: &str) -> Vec<Uuid> {
        let mut rx = self.app.state.broker.subscribe();
        let (status, body) = self
            .app
            .post(
                &format!("/api/channels/{}/messages", self.channel),
                &auth("alice"),
                json!({ "content": content }),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        let mut notified = Vec::new();
        while let Ok(event) = rx.try_recv() {
            if let ServerEvent::NotificationCreated { recipient_id, .. } = event {
                notified.push(burst_core::id::parse_prefixed_id(&recipient_id, "usr_").unwrap());
            }
        }
        notified
    }
}

fn in_minutes(minutes: i64) -> String {
    (Utc::now() + Duration::minutes(minutes)).to_rfc3339()
}

/// Quiet hours around the current time, in UTC, every day.
fn schedule_around_now() -> Value {
    let now = Utc::now();
    json!({
        "start": (now - Duration::hours(1)).format("%H:%M").to_string(),
        "end": (now + Duration::hours(1)).format("%H:%M").to_string(),
        "days": ALL_DAYS,
        "timeZone": "UTC",
    })
}

/// Quiet hours that exclude the current time, every day.
fn schedule_away_from_now() -> Value {
    let now = Utc::now();
    json!({
        "start": (now + Duration::hours(2)).format("%H:%M").to_string(),
        "end": (now + Duration::hours(3)).format("%H:%M").to_string(),
        "days": ALL_DAYS,
        "timeZone": "UTC",
    })
}

// ── Delivery ─────────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn a_snoozed_member_is_not_notified(pool: sqlx::PgPool) {
    let s = setup(&pool).await;
    assert_eq!(
        s.set("bob", json!({ "snoozeUntil": in_minutes(60) }))
            .await
            .0,
        StatusCode::OK
    );

    let notified = s.post("hello").await;
    assert!(
        notified.contains(&s.carol),
        "control: carol is notified: {notified:?}"
    );
    assert!(!notified.contains(&s.bob), "{notified:?}");
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_mention_is_held_back_too(pool: sqlx::PgPool) {
    let s = setup(&pool).await;
    s.set("bob", json!({ "snoozeUntil": in_minutes(60) })).await;

    let notified = s.post("@bob @carol look").await;
    assert!(notified.contains(&s.carol), "control: {notified:?}");
    assert!(!notified.contains(&s.bob), "{notified:?}");
}

#[sqlx::test(migrations = "../../migrations")]
async fn quiet_hours_hold_back_notifications_only_while_they_run(pool: sqlx::PgPool) {
    let s = setup(&pool).await;
    s.set("bob", json!({ "schedule": schedule_away_from_now() }))
        .await;
    assert!(
        s.post("before").await.contains(&s.bob),
        "outside the window bob is notified"
    );

    s.set("bob", json!({ "schedule": schedule_around_now() }))
        .await;
    let notified = s.post("during").await;
    assert!(notified.contains(&s.carol), "control: {notified:?}");
    assert!(!notified.contains(&s.bob), "{notified:?}");
}

#[sqlx::test(migrations = "../../migrations")]
async fn notifications_resume_when_the_snooze_ends(pool: sqlx::PgPool) {
    let s = setup(&pool).await;
    s.set("bob", json!({ "snoozeUntil": in_minutes(60) })).await;
    assert!(!s.post("quiet").await.contains(&s.bob), "control: snoozed");

    sqlx::query("UPDATE users SET dnd_until = now() - interval '1 second' WHERE id = $1")
        .bind(s.bob)
        .execute(&s.pool)
        .await
        .unwrap();
    assert!(s.post("awake").await.contains(&s.bob));
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_quiet_member_still_sees_the_message_as_unread(pool: sqlx::PgPool) {
    let s = setup(&pool).await;
    s.set("bob", json!({ "snoozeUntil": in_minutes(60) })).await;
    s.post("while you were away").await;

    let (_, channels) = s.app.get("/api/channels?joined=true", &auth("bob")).await;
    let general = channels["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == s.channel.as_str())
        .unwrap();
    assert_eq!(general["unreadCount"], 1, "{general}");
}

// ── Settings ─────────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn settings_round_trip_and_report_the_quiet_period(pool: sqlx::PgPool) {
    let s = setup(&pool).await;
    let schedule = json!({ "start": "22:00", "end": "07:00", "days": ["fri", "mon"], "timeZone": "Europe/Paris" });
    let until = in_minutes(30);
    let (status, body) = s
        .set("bob", json!({ "snoozeUntil": until, "schedule": schedule }))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (_, read) = s.app.get(DND, &auth("bob")).await;
    assert_eq!(
        read["schedule"],
        json!({ "start": "22:00", "end": "07:00", "days": ["mon", "fri"], "timeZone": "Europe/Paris" })
    );
    let snooze: chrono::DateTime<Utc> = read["snoozeUntil"].as_str().unwrap().parse().unwrap();
    assert_eq!(
        snooze.timestamp(),
        until.parse::<chrono::DateTime<Utc>>().unwrap().timestamp()
    );
    assert!(read["quietUntil"].is_string(), "snoozed now: {read}");
}

#[sqlx::test(migrations = "../../migrations")]
async fn setting_replaces_the_whole_setting(pool: sqlx::PgPool) {
    let s = setup(&pool).await;
    s.set(
        "bob",
        json!({ "snoozeUntil": in_minutes(30), "schedule": schedule_around_now() }),
    )
    .await;

    let (status, body) = s.set("bob", json!({})).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({}), "an empty setting turns it all off");
    assert!(s.post("back").await.contains(&s.bob));
}

#[sqlx::test(migrations = "../../migrations")]
async fn invalid_settings_are_refused_and_the_old_ones_kept(pool: sqlx::PgPool) {
    let s = setup(&pool).await;
    s.set("bob", json!({ "snoozeUntil": in_minutes(60) })).await;

    let base = json!({ "start": "22:00", "end": "07:00", "days": ["mon"], "timeZone": "UTC" });
    let with = |key: &str, value: Value| {
        let mut schedule = base.clone();
        schedule[key] = value;
        json!({ "schedule": schedule })
    };
    for body in [
        json!({ "snoozeUntil": in_minutes(-1) }),
        json!({ "snoozeUntil": "soon" }),
        with("start", json!("7am")),
        with("end", json!("22:00")),
        with("days", json!([])),
        with("days", json!(["someday"])),
        with("timeZone", json!("Mars/Olympus")),
    ] {
        let (status, response) = s.set("bob", body.clone()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body} gave {response}");
    }
    let (_, read) = s.app.get(DND, &auth("bob")).await;
    assert!(read["snoozeUntil"].is_string(), "{read}");
}

#[sqlx::test(migrations = "../../migrations")]
async fn others_see_when_a_user_is_quiet_and_are_told_live(pool: sqlx::PgPool) {
    let s = setup(&pool).await;
    let bob_id = burst_core::id::format_user_id(s.bob);
    let bob_uri = format!("/api/users/{bob_id}");
    assert!(
        s.app
            .get(&bob_uri, &auth("carol"))
            .await
            .1
            .get("doNotDisturbUntil")
            .is_none()
    );

    let mut rx = s.app.state.broker.subscribe();
    s.set("bob", json!({ "snoozeUntil": in_minutes(60) })).await;
    assert!(s.app.get(&bob_uri, &auth("carol")).await.1["doNotDisturbUntil"].is_string());
    let announced = std::iter::from_fn(|| rx.try_recv().ok()).find_map(|e| match e {
        ServerEvent::UserDndChanged { user_id, until, .. } if user_id == bob_id => Some(until),
        _ => None,
    });
    assert!(matches!(announced, Some(Some(_))), "{announced:?}");

    let mut rx = s.app.state.broker.subscribe();
    s.set("bob", json!({})).await;
    let announced = std::iter::from_fn(|| rx.try_recv().ok()).find_map(|e| match e {
        ServerEvent::UserDndChanged { user_id, until, .. } if user_id == bob_id => Some(until),
        _ => None,
    });
    assert_eq!(announced, Some(None));
}
