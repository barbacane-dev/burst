//! Data export: requesting, building, downloading and deleting an archive.

mod common;

use std::collections::HashMap;
use std::io::Read;
use std::time::Duration;

use axum::http::StatusCode;
use burst_core::id::{format_attachment_id, format_channel_id, format_message_id, format_user_id};
use burst_server::db;
use serde_json::{Value, json};
use uuid::Uuid;

fn auth(username: &str) -> String {
    format!("{username}@test.example")
}

struct World {
    app: common::TestApp,
    pool: sqlx::PgPool,
    alice: Uuid,
    bob: Uuid,
    carol: Uuid,
    general: Uuid,
    secret: Uuid,
    dm: Uuid,
}

async fn world(pool: &sqlx::PgPool) -> World {
    let app = common::TestApp::new(pool.clone());
    common::seed_user_with_role(pool, "root", "admin").await;
    let alice = common::seed_user(pool, "alice").await.id;
    let bob = common::seed_user(pool, "bob").await.id;
    let carol = common::seed_user(pool, "carol").await.id;
    let general = common::seed_channel(pool, "general", alice).await.id;
    db::channels::add_member(pool, general, bob, "member")
        .await
        .unwrap();

    let secret = burst_core::id::new_id();
    db::channels::create(
        pool,
        &db::channels::CreateChannel {
            id: secret,
            kind: "private".into(),
            name: Some("secret".into()),
            slug: Some("secret".into()),
            topic: None,
            description: None,
            created_by: carol,
        },
    )
    .await
    .unwrap();
    db::channels::add_member(pool, secret, carol, "owner")
        .await
        .unwrap();
    let dm = db::channels::find_or_create_dm(pool, bob, carol, burst_core::id::new_id())
        .await
        .unwrap()
        .id;

    World {
        app,
        pool: pool.clone(),
        alice,
        bob,
        carol,
        general,
        secret,
        dm,
    }
}

impl World {
    async fn message(&self, channel: Uuid, author: Uuid, content: &str, at: &str) -> Uuid {
        let id = burst_core::id::new_id();
        sqlx::query(
            "INSERT INTO messages (id, channel_id, user_id, content, created_at) \
             VALUES ($1, $2, $3, $4, $5::timestamptz)",
        )
        .bind(id)
        .bind(channel)
        .bind(author)
        .bind(content)
        .bind(at)
        .execute(&self.pool)
        .await
        .unwrap();
        id
    }

    async fn attach(&self, message: Uuid, name: &str, data: &'static [u8], stored: bool) -> Uuid {
        let id = burst_core::id::new_id();
        let key = format!("att/{id}");
        if stored {
            self.app
                .state
                .storage
                .put(&key, bytes::Bytes::from_static(data), "text/plain")
                .await
                .unwrap();
        }
        db::attachments::create(
            &self.pool,
            &db::attachments::CreateAttachment {
                id,
                message_id: message,
                file_name: name.into(),
                file_size: data.len() as i64,
                content_type: "text/plain".into(),
                storage_key: key,
                metadata: json!({}),
            },
        )
        .await
        .unwrap();
        id
    }

    async fn request(&self, body: Value) -> (StatusCode, Value) {
        self.app
            .post("/api/admin/exports", &auth("root"), body)
            .await
    }

    /// Requests an export and waits for it to finish.
    async fn export(&self, body: Value) -> Value {
        let (status, created) = self.request(body).await;
        assert_eq!(status, StatusCode::ACCEPTED, "{created}");
        let id = created["id"].as_str().unwrap().to_string();
        for _ in 0..100 {
            let (_, export) = self
                .app
                .get(&format!("/api/admin/exports/{id}"), &auth("root"))
                .await;
            match export["status"].as_str() {
                Some("completed") | Some("failed") => return export,
                _ => tokio::time::sleep(Duration::from_millis(50)).await,
            }
        }
        panic!("export {id} did not finish");
    }

    async fn download(&self, id: &str) -> Archive {
        let (status, headers, body) = self
            .app
            .get_raw(&format!("/api/admin/exports/{id}/download"), &auth("root"))
            .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers["content-type"], "application/zip");
        assert!(
            headers["content-disposition"]
                .to_str()
                .unwrap()
                .starts_with("attachment; filename=\"burst-export-")
        );
        Archive::read(&body)
    }
}

/// Every entry of a zip, by name.
struct Archive(HashMap<String, Vec<u8>>);

impl Archive {
    fn read(bytes: &[u8]) -> Self {
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("a zip archive");
        let mut entries = HashMap::new();
        for i in 0..zip.len() {
            let mut file = zip.by_index(i).unwrap();
            let mut data = Vec::new();
            file.read_to_end(&mut data).unwrap();
            entries.insert(file.name().to_string(), data);
        }
        Self(entries)
    }

    fn json(&self, name: &str) -> Value {
        serde_json::from_slice(
            self.0
                .get(name)
                .unwrap_or_else(|| panic!("{name} missing: {:?}", self.names())),
        )
        .unwrap()
    }

    fn lines(&self, channel: Uuid) -> Vec<Value> {
        let name = format!("channels/{}/messages.jsonl", format_channel_id(channel));
        let data = self
            .0
            .get(&name)
            .unwrap_or_else(|| panic!("{name} missing: {:?}", self.names()));
        std::str::from_utf8(data)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    fn names(&self) -> Vec<&String> {
        let mut names: Vec<_> = self.0.keys().collect();
        names.sort();
        names
    }

    fn channel_dirs(&self) -> Vec<String> {
        let mut dirs: Vec<String> = self
            .0
            .keys()
            .filter_map(|n| n.strip_suffix("/channel.json"))
            .map(|d| d.trim_start_matches("channels/").to_string())
            .collect();
        dirs.sort();
        dirs
    }
}

// ── The archive ──────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn an_instance_export_holds_every_channel_message_and_file(pool: sqlx::PgPool) {
    let w = world(&pool).await;
    let first = w
        .message(w.general, w.alice, "first", "2026-09-01T10:00:00Z")
        .await;
    let reply = w
        .message(w.general, w.bob, "a reply", "2026-09-01T10:05:00Z")
        .await;
    sqlx::query("UPDATE messages SET thread_id = $1 WHERE id = $2")
        .bind(first)
        .bind(reply)
        .execute(&pool)
        .await
        .unwrap();
    let gone = w
        .message(w.general, w.bob, "deleted", "2026-09-01T10:10:00Z")
        .await;
    sqlx::query("UPDATE messages SET deleted_at = now() WHERE id = $1")
        .bind(gone)
        .execute(&pool)
        .await
        .unwrap();
    let with_file = w
        .message(w.general, w.alice, "the plan", "2026-09-01T11:00:00Z")
        .await;
    let file = w.attach(with_file, "plan.txt", b"step one", true).await;
    for (user, emoji) in [(w.alice, "👍"), (w.bob, "🎉"), (w.bob, "👍")] {
        db::reactions::add(&pool, with_file, user, emoji)
            .await
            .unwrap();
    }
    w.message(w.secret, w.carol, "private note", "2026-09-02T09:00:00Z")
        .await;
    w.message(w.dm, w.bob, "hi carol", "2026-09-02T09:30:00Z")
        .await;

    let export = w.export(json!({})).await;
    assert_eq!(export["status"], "completed", "{export}");
    assert_eq!(export["scope"], "instance");
    assert!(export["sizeBytes"].as_i64().unwrap() > 0);
    let archive = w.download(export["id"].as_str().unwrap()).await;

    let manifest = archive.json("export.json");
    assert_eq!(manifest["format"], "burst-export/1");
    assert_eq!(manifest["channels"], 3);
    assert_eq!(
        manifest["messages"], 5,
        "the deleted message is left out: {manifest}"
    );
    assert_eq!(manifest["files"], 1);

    let mut expected = vec![
        format_channel_id(w.general),
        format_channel_id(w.secret),
        format_channel_id(w.dm),
    ];
    expected.sort();
    assert_eq!(
        archive.channel_dirs(),
        expected,
        "private channels and DMs included"
    );

    let lines = archive.lines(w.general);
    let ids: Vec<&str> = lines.iter().map(|l| l["id"].as_str().unwrap()).collect();
    assert_eq!(
        ids,
        [first, reply, with_file].map(format_message_id),
        "oldest first, no deleted message"
    );
    assert_eq!(lines[1]["threadId"], format_message_id(first));
    assert_eq!(lines[0]["userId"], format_user_id(w.alice));

    let attachment = &lines[2]["attachments"][0];
    let path = format!("files/{}/plan.txt", format_attachment_id(file));
    assert_eq!(attachment["path"], path.as_str());
    assert_eq!(archive.0[&path], b"step one");
    assert_eq!(
        lines[2]["reactions"],
        json!([
            { "emoji": "👍", "userIds": [format_user_id(w.alice), format_user_id(w.bob)] },
            { "emoji": "🎉", "userIds": [format_user_id(w.bob)] },
        ])
    );

    let channel = archive.json(&format!(
        "channels/{}/channel.json",
        format_channel_id(w.general)
    ));
    assert_eq!(channel["name"], "general");
    assert_eq!(channel["members"].as_array().unwrap().len(), 2);

    let users = archive.json("users.json");
    assert_eq!(users.as_array().unwrap().len(), 4, "every user: {users}");
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_channel_export_holds_that_channel_and_its_people_only(pool: sqlx::PgPool) {
    let w = world(&pool).await;
    w.message(w.general, w.alice, "in general", "2026-09-01T10:00:00Z")
        .await;
    w.message(w.secret, w.carol, "elsewhere", "2026-09-01T10:00:00Z")
        .await;

    let export = w
        .export(json!({ "channelId": format_channel_id(w.general) }))
        .await;
    assert_eq!(export["scope"], "channel");
    assert_eq!(export["channelId"], format_channel_id(w.general));
    let archive = w.download(export["id"].as_str().unwrap()).await;

    assert_eq!(archive.channel_dirs(), vec![format_channel_id(w.general)]);
    let users: Vec<String> = archive
        .json("users.json")
        .as_array()
        .unwrap()
        .iter()
        .map(|u| u["username"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(users.len(), 2, "{users:?}");
    assert!(
        users.contains(&"alice".to_string()) && users.contains(&"bob".to_string()),
        "{users:?}"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_file_missing_from_storage_is_listed_not_fatal(pool: sqlx::PgPool) {
    let w = world(&pool).await;
    let msg = w
        .message(w.general, w.alice, "files", "2026-09-01T10:00:00Z")
        .await;
    let kept = w.attach(msg, "kept.txt", b"here", true).await;
    let lost = w.attach(msg, "lost.txt", b"gone", false).await;

    let export = w.export(json!({})).await;
    assert_eq!(export["status"], "completed", "{export}");
    let archive = w.download(export["id"].as_str().unwrap()).await;

    assert_eq!(
        archive.json("export.json")["missingFiles"],
        json!([format_attachment_id(lost)])
    );
    let attachments = archive.lines(w.general)[0]["attachments"]
        .as_array()
        .unwrap()
        .clone();
    let by_id = |id: Uuid| {
        attachments
            .iter()
            .find(|a| a["id"] == format_attachment_id(id))
            .unwrap()
            .clone()
    };
    assert!(by_id(lost).get("path").is_none(), "{attachments:?}");
    assert!(by_id(kept)["path"].is_string(), "{attachments:?}");
}

#[sqlx::test(migrations = "../../migrations")]
async fn file_names_cannot_escape_the_archive(pool: sqlx::PgPool) {
    let w = world(&pool).await;
    let msg = w
        .message(w.general, w.alice, "sneaky", "2026-09-01T10:00:00Z")
        .await;
    let file = w.attach(msg, "../../etc/passwd", b"x", true).await;

    let export = w.export(json!({})).await;
    let archive = w.download(export["id"].as_str().unwrap()).await;
    let expected = format!("files/{}/passwd", format_attachment_id(file));
    assert!(archive.0.contains_key(&expected), "{:?}", archive.names());
    assert!(
        archive.names().iter().all(|n| !n.contains("..")),
        "{:?}",
        archive.names()
    );
}

// ── Lifecycle ────────────────────────────────────────────────────────────────

async fn pending_row(w: &World, updated_minutes_ago: i64) -> Uuid {
    let id = burst_core::id::new_id();
    let admin = db::users::find_by_external_id(&w.pool, &auth("root"))
        .await
        .unwrap()
        .unwrap()
        .id;
    sqlx::query(
        "INSERT INTO exports (id, requested_by, scope, status, updated_at) \
         VALUES ($1, $2, 'instance', 'running', now() - make_interval(mins => $3::int))",
    )
    .bind(id)
    .bind(admin)
    .bind(updated_minutes_ago as i32)
    .execute(&w.pool)
    .await
    .unwrap();
    id
}

#[sqlx::test(migrations = "../../migrations")]
async fn one_export_runs_at_a_time_and_a_stalled_one_is_failed(pool: sqlx::PgPool) {
    let w = world(&pool).await;
    let live = pending_row(&w, 1).await;
    assert_eq!(w.request(json!({})).await.0, StatusCode::CONFLICT);

    sqlx::query("UPDATE exports SET updated_at = now() - interval '11 minutes' WHERE id = $1")
        .bind(live)
        .execute(&pool)
        .await
        .unwrap();
    let (_, stalled) = w
        .app
        .get(&format!("/api/admin/exports/exp_{live}"), &auth("root"))
        .await;
    assert_eq!(stalled["status"], "failed");
    assert_eq!(stalled["error"], "interrupted");
    assert_eq!(w.export(json!({})).await["status"], "completed");
}

#[sqlx::test(migrations = "../../migrations")]
async fn an_unfinished_export_cannot_be_downloaded_or_deleted(pool: sqlx::PgPool) {
    let w = world(&pool).await;
    let running = pending_row(&w, 0).await;
    let (status, _, _) = w
        .app
        .get_raw(
            &format!("/api/admin/exports/exp_{running}/download"),
            &auth("root"),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        w.app
            .delete(&format!("/api/admin/exports/exp_{running}"), &auth("root"))
            .await
            .0,
        StatusCode::CONFLICT
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn deleting_removes_the_archive(pool: sqlx::PgPool) {
    let w = world(&pool).await;
    let export = w.export(json!({})).await;
    let id = export["id"].as_str().unwrap();
    let key = format!("exports/{}.zip", id.trim_start_matches("exp_"));
    assert!(
        w.app.state.storage.get(&key).await.is_ok(),
        "control: the archive is stored"
    );

    assert_eq!(
        w.app
            .delete(&format!("/api/admin/exports/{id}"), &auth("root"))
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert!(w.app.state.storage.get(&key).await.is_err());
    assert_eq!(
        w.app
            .get(&format!("/api/admin/exports/{id}"), &auth("root"))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let (_, list) = w.app.get("/api/admin/exports", &auth("root")).await;
    assert!(list["items"].as_array().unwrap().is_empty());
}

#[sqlx::test(migrations = "../../migrations")]
async fn requests_and_downloads_are_audited(pool: sqlx::PgPool) {
    let w = world(&pool).await;
    let export = w
        .export(json!({ "channelId": format_channel_id(w.general) }))
        .await;
    w.download(export["id"].as_str().unwrap()).await;

    let actions: Vec<(String, Option<Value>)> =
        sqlx::query_as("SELECT action, metadata FROM audit_log ORDER BY created_at")
            .fetch_all(&pool)
            .await
            .unwrap();
    let requested = actions
        .iter()
        .find(|(a, _)| a == "export.requested")
        .expect("request audited");
    assert_eq!(
        requested.1.as_ref().unwrap()["channelId"],
        format_channel_id(w.general)
    );
    assert!(
        actions.iter().any(|(a, _)| a == "export.downloaded"),
        "{actions:?}"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn only_admins_can_export(pool: sqlx::PgPool) {
    let w = world(&pool).await;
    let export = w.export(json!({})).await;
    let id = export["id"].as_str().unwrap();
    let _ = (w.alice, w.bob);

    assert_eq!(
        w.app
            .post("/api/admin/exports", &auth("alice"), json!({}))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        w.app.get("/api/admin/exports", &auth("alice")).await.0,
        StatusCode::FORBIDDEN
    );
    let (status, _, _) = w
        .app
        .get_raw(&format!("/api/admin/exports/{id}/download"), &auth("alice"))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(
        w.app
            .delete(&format!("/api/admin/exports/{id}"), &auth("alice"))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn an_unknown_channel_or_id_is_refused(pool: sqlx::PgPool) {
    let w = world(&pool).await;
    let missing = format_channel_id(burst_core::id::new_id());
    assert_eq!(
        w.request(json!({ "channelId": missing })).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        w.request(json!({ "channelId": "nope" })).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        w.app.get("/api/admin/exports/nope", &auth("root")).await.0,
        StatusCode::BAD_REQUEST
    );
}

/// More than two pages of messages, with ids in the order they were numbered:
/// every one is exported once, in order.
#[sqlx::test(migrations = "../../migrations")]
async fn a_long_channel_is_exported_whole(pool: sqlx::PgPool) {
    let w = world(&pool).await;
    sqlx::query(
        "INSERT INTO messages (id, channel_id, user_id, content, created_at) \
         SELECT (lpad(to_hex(n), 8, '0') || '-0000-7000-8000-000000000000')::uuid, \
                $1, $2, 'message ' || n, now() \
         FROM generate_series(1, 1201) AS n",
    )
    .bind(w.general)
    .bind(w.alice)
    .execute(&pool)
    .await
    .unwrap();

    let export = w
        .export(json!({ "channelId": format_channel_id(w.general) }))
        .await;
    let lines = w
        .download(export["id"].as_str().unwrap())
        .await
        .lines(w.general);
    assert_eq!(lines.len(), 1201);
    let contents: Vec<&str> = lines
        .iter()
        .map(|l| l["content"].as_str().unwrap())
        .collect();
    assert_eq!(contents[0], "message 1");
    assert_eq!(contents[500], "message 501");
    assert_eq!(contents[1200], "message 1201");
}
