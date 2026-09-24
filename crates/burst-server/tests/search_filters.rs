//! Search filters, file names, and pages that continue where the last ended.

mod common;

use std::collections::HashSet;

use axum::http::StatusCode;
use burst_server::db;
use serde_json::Value;
use uuid::Uuid;

fn auth(username: &str) -> String {
    format!("{username}@test.example")
}

struct Setup {
    app: common::TestApp,
    pool: sqlx::PgPool,
    general: Uuid,
    alice: Uuid,
    bob: Uuid,
}

async fn setup(pool: &sqlx::PgPool) -> Setup {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(pool, "alice").await.id;
    let bob = common::seed_user(pool, "bob").await.id;
    let general = common::seed_channel(pool, "general", alice).await.id;
    db::channels::add_member(pool, general, bob, "member")
        .await
        .unwrap();
    Setup {
        app,
        pool: pool.clone(),
        general,
        alice,
        bob,
    }
}

impl Setup {
    /// Inserts a message directly, so its time can be set.
    async fn message(&self, author: Uuid, content: &str, at: &str) -> Uuid {
        let id = burst_core::id::new_id();
        sqlx::query(
            "INSERT INTO messages (id, channel_id, user_id, content, created_at) \
             VALUES ($1, $2, $3, $4, $5::timestamptz)",
        )
        .bind(id)
        .bind(self.general)
        .bind(author)
        .bind(content)
        .bind(at)
        .execute(&self.pool)
        .await
        .unwrap();
        id
    }

    async fn attach(&self, message: Uuid, file_name: &str) {
        db::attachments::create(
            &self.pool,
            &db::attachments::CreateAttachment {
                id: burst_core::id::new_id(),
                message_id: message,
                file_name: file_name.into(),
                file_size: 10,
                content_type: "application/pdf".into(),
                storage_key: format!("k/{file_name}"),
                metadata: serde_json::json!({}),
            },
        )
        .await
        .unwrap();
    }

    async fn search(&self, as_user: &str, query: &str) -> (StatusCode, Value) {
        self.app
            .get(&format!("/api/search/messages?{query}"), &auth(as_user))
            .await
    }

    async fn ids(&self, query: &str) -> Vec<Uuid> {
        let (status, body) = self.search("alice", query).await;
        assert_eq!(status, StatusCode::OK, "{query}: {body}");
        ids_of(&body)
    }
}

fn ids_of(body: &Value) -> Vec<Uuid> {
    body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| burst_core::id::parse_prefixed_id(m["id"].as_str().unwrap(), "msg_").unwrap())
        .collect()
}

fn set(ids: &[Uuid]) -> HashSet<Uuid> {
    ids.iter().copied().collect()
}

// ── Filters ──────────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn from_keeps_one_authors_messages(pool: sqlx::PgPool) {
    let s = setup(&pool).await;
    let by_alice = s
        .message(s.alice, "release notes draft", "2026-09-10T10:00:00Z")
        .await;
    let by_bob = s
        .message(s.bob, "release checklist", "2026-09-10T11:00:00Z")
        .await;

    assert_eq!(
        set(&s.ids("q=release").await),
        set(&[by_alice, by_bob]),
        "control"
    );
    let from_bob = format!("q=release&from={}", burst_core::id::format_user_id(s.bob));
    assert_eq!(s.ids(&from_bob).await, vec![by_bob]);
}

#[sqlx::test(migrations = "../../migrations")]
async fn after_is_inclusive_and_before_is_exclusive(pool: sqlx::PgPool) {
    let s = setup(&pool).await;
    let early = s
        .message(s.alice, "deploy one", "2026-09-01T09:00:00Z")
        .await;
    let middle = s
        .message(s.alice, "deploy two", "2026-09-15T00:00:00Z")
        .await;
    let late = s
        .message(s.alice, "deploy three", "2026-09-30T00:00:00Z")
        .await;

    let window = "q=deploy&after=2026-09-15T00:00:00Z&before=2026-09-30T00:00:00Z";
    assert_eq!(s.ids(window).await, vec![middle]);
    assert_eq!(
        set(&s.ids("q=deploy").await),
        set(&[early, middle, late]),
        "control"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_timezone_offset_is_honoured(pool: sqlx::PgPool) {
    let s = setup(&pool).await;
    // 23:30 UTC on the 14th is already the 15th in Paris.
    let msg = s
        .message(s.alice, "late deploy", "2026-09-14T23:30:00Z")
        .await;
    assert_eq!(
        s.ids("q=deploy&after=2026-09-15T00:00:00%2B02:00").await,
        vec![msg]
    );
    assert!(
        s.ids("q=deploy&after=2026-09-15T00:00:00Z")
            .await
            .is_empty()
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn an_empty_or_backwards_window_is_refused(pool: sqlx::PgPool) {
    let s = setup(&pool).await;
    for q in [
        "q=x&after=2026-09-30T00:00:00Z&before=2026-09-01T00:00:00Z",
        "q=x&after=2026-09-01T00:00:00Z&before=2026-09-01T00:00:00Z",
    ] {
        assert_eq!(s.search("alice", q).await.0, StatusCode::BAD_REQUEST, "{q}");
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn has_file_keeps_messages_with_attachments(pool: sqlx::PgPool) {
    let s = setup(&pool).await;
    let plain = s
        .message(s.alice, "budget figures", "2026-09-10T10:00:00Z")
        .await;
    let with_file = s
        .message(s.alice, "budget attached", "2026-09-10T11:00:00Z")
        .await;
    s.attach(with_file, "numbers.xlsx").await;

    assert_eq!(
        set(&s.ids("q=budget").await),
        set(&[plain, with_file]),
        "control"
    );
    assert_eq!(s.ids("q=budget&hasFile=true").await, vec![with_file]);
}

#[sqlx::test(migrations = "../../migrations")]
async fn malformed_filters_are_refused(pool: sqlx::PgPool) {
    let s = setup(&pool).await;
    for q in [
        "q=x&from=nobody",
        "q=x&after=yesterday",
        "q=x&before=2026-13-01",
        "q=x&cursor=garbage",
    ] {
        assert_eq!(s.search("alice", q).await.0, StatusCode::BAD_REQUEST, "{q}");
    }
}

// ── File names ───────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn a_file_name_is_found_and_named(pool: sqlx::PgPool) {
    let s = setup(&pool).await;
    let msg = s
        .message(s.alice, "here you go", "2026-09-10T10:00:00Z")
        .await;
    s.attach(msg, "Q3-Report-final.pdf").await;
    let text_match = s
        .message(s.alice, "the report is late", "2026-09-10T11:00:00Z")
        .await;

    let (_, body) = s.search("alice", "q=report").await;
    let items = body["items"].as_array().unwrap();
    let by_id = |id: Uuid| {
        items
            .iter()
            .find(|m| m["id"] == burst_core::id::format_message_id(id))
            .unwrap_or_else(|| panic!("{id} missing from {body}"))
    };
    assert_eq!(by_id(msg)["matchedFile"], "Q3-Report-final.pdf");
    assert!(
        by_id(text_match).get("matchedFile").is_none(),
        "a text match names no file"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_file_in_a_channel_the_caller_is_not_in_stays_hidden(pool: sqlx::PgPool) {
    let s = setup(&pool).await;
    common::seed_user(&pool, "carol").await;
    let msg = s.message(s.alice, "attached", "2026-09-10T10:00:00Z").await;
    s.attach(msg, "salaries-2026.csv").await;

    assert_eq!(
        s.ids("q=salaries").await,
        vec![msg],
        "control: a member finds it"
    );
    let (status, body) = s.search("carol", "q=salaries").await;
    assert_eq!(status, StatusCode::OK);
    assert!(ids_of(&body).is_empty(), "{body}");
}

#[sqlx::test(migrations = "../../migrations")]
async fn wildcards_in_the_query_match_literally(pool: sqlx::PgPool) {
    let s = setup(&pool).await;
    let plain = s.message(s.alice, "files", "2026-09-10T10:00:00Z").await;
    s.attach(plain, "summary.pdf").await;
    let percent = s
        .message(s.alice, "more files", "2026-09-10T11:00:00Z")
        .await;
    s.attach(percent, "discount-50%.pdf").await;
    let underscore = s.message(s.alice, "and more", "2026-09-10T12:00:00Z").await;
    s.attach(underscore, "draft_v2.pdf").await;

    // Passed through as wildcards, `%` and `_` would each match every name.
    assert_eq!(s.ids("q=%25").await, vec![percent]);
    assert_eq!(s.ids("q=_").await, vec![underscore]);
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_deleted_messages_file_is_not_found(pool: sqlx::PgPool) {
    let s = setup(&pool).await;
    let msg = s.message(s.alice, "old", "2026-09-10T10:00:00Z").await;
    s.attach(msg, "obsolete-plan.pdf").await;
    assert_eq!(s.ids("q=obsolete").await, vec![msg], "control");

    sqlx::query("UPDATE messages SET deleted_at = now() WHERE id = $1")
        .bind(msg)
        .execute(&pool)
        .await
        .unwrap();
    assert!(s.ids("q=obsolete").await.is_empty());
}

// ── Paging ───────────────────────────────────────────────────────────────────

/// Relevance order and id order disagree here: the oldest message, which has
/// the lowest id, is the most relevant. A cursor that pages by id alone would
/// stop after it and lose the rest.
#[sqlx::test(migrations = "../../migrations")]
async fn pages_continue_where_the_last_ended_when_relevance_and_age_disagree(pool: sqlx::PgPool) {
    let s = setup(&pool).await;
    let mut all = Vec::new();
    for (i, text) in [
        "incident incident incident incident incident",
        "incident incident incident",
        "incident incident",
        "incident",
        "one incident among many other words in a longer message",
    ]
    .iter()
    .enumerate()
    {
        all.push(
            s.message(s.alice, text, &format!("2026-09-10T1{i}:00:00Z"))
                .await,
        );
    }

    let mut seen = Vec::new();
    let mut cursor: Option<String> = None;
    for _ in 0..10 {
        let q = match &cursor {
            Some(c) => format!("q=incident&limit=2&cursor={c}"),
            None => "q=incident&limit=2".into(),
        };
        let (status, body) = s.search("alice", &q).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        seen.extend(ids_of(&body));
        cursor = body["cursor"].as_str().map(String::from);
        if cursor.is_none() {
            break;
        }
    }

    assert_eq!(
        seen.len(),
        set(&seen).len(),
        "a result appeared twice: {seen:?}"
    );
    assert_eq!(set(&seen), set(&all), "results were skipped");
    assert_eq!(seen[0], all[0], "the most relevant comes first");
}

/// The gateway validates query parameters against the spec, so a cursor the
/// server issues must match the pattern the spec declares for it. Tests reach
/// the server directly and would not notice otherwise.
#[sqlx::test(migrations = "../../migrations")]
async fn an_issued_cursor_satisfies_the_pattern_the_spec_declares(pool: sqlx::PgPool) {
    let s = setup(&pool).await;
    for i in 0..4 {
        s.message(
            s.alice,
            &"match ".repeat(i + 1),
            &format!("2026-09-10T1{i}:00:00Z"),
        )
        .await;
    }
    let (_, body) = s.search("alice", "q=match&limit=1").await;
    let cursor = body["cursor"].as_str().expect("a cursor for the next page");

    let spec: serde_yaml::Value = serde_yaml::from_str(
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../specs/burst-api.yaml"
        ))
        .unwrap(),
    )
    .unwrap();
    let pattern = spec["paths"]["/api/search/messages"]["get"]["parameters"]
        .as_sequence()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "cursor")
        .and_then(|p| p["schema"]["pattern"].as_str())
        .expect("the spec declares a cursor pattern");
    assert!(
        regex::Regex::new(pattern).unwrap().is_match(cursor),
        "cursor {cursor:?} would be rejected by the gateway: it does not match {pattern:?}"
    );
}
