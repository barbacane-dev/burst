#![allow(dead_code)] // helpers are shared utilities; not every test file uses all of them

/// Shared test harness for burst-server integration tests.
///
/// Provides `TestApp` (wraps the Axum router with a test database) and seed
/// helpers for creating users and channels.  All public items here are
/// available to every file inside `tests/`.
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

use burst_server::{
    AppState, admin_router, app_router,
    config::{AppConfig, StorageConfig, WebSocketConfig},
    db, metrics,
    storage::{Storage, local::LocalStorage},
};

// ── TestApp ───────────────────────────────────────────────────────────────────

pub struct TestApp {
    /// The Axum router under test.  Clone it for each `oneshot` call.
    pub router: axum::Router,
    /// The admin router (health + metrics endpoints).
    pub admin: axum::Router,
    /// The full app state — useful for inspecting the broker, presence, etc.
    pub state: AppState,
    /// Temp dir backing the local storage (kept alive for the test lifetime).
    _storage_dir: tempfile::TempDir,
}

impl TestApp {
    pub fn new(pool: PgPool) -> Self {
        let storage_dir = tempfile::tempdir().expect("failed to create temp storage dir");
        let config = AppConfig {
            storage: StorageConfig {
                local_path: storage_dir.path().to_string_lossy().into_owned(),
                ..StorageConfig::default()
            },
            websocket: WebSocketConfig::default(),
        };
        let storage = Storage::Local(LocalStorage::new(storage_dir.path().to_path_buf()).unwrap());
        let state = AppState::new(
            pool,
            config,
            storage,
            metrics::noop(),
            tokio_util::sync::CancellationToken::new(),
        );
        let router = app_router(state.clone());
        let admin = admin_router(state.clone());
        Self {
            router,
            admin,
            state,
            _storage_dir: storage_dir,
        }
    }

    /// POST `uri` with a JSON body, authenticated via external_id.
    pub async fn post(
        &self,
        uri: &str,
        external_id: &str,
        body: serde_json::Value,
    ) -> (StatusCode, serde_json::Value) {
        self.request("POST", uri, Some(external_id), Some(body))
            .await
    }

    /// GET `uri` authenticated via external_id.
    pub async fn get(&self, uri: &str, external_id: &str) -> (StatusCode, serde_json::Value) {
        self.request("GET", uri, Some(external_id), None).await
    }

    /// PATCH `uri` with a JSON body, authenticated via external_id.
    pub async fn patch(
        &self,
        uri: &str,
        external_id: &str,
        body: serde_json::Value,
    ) -> (StatusCode, serde_json::Value) {
        self.request("PATCH", uri, Some(external_id), Some(body))
            .await
    }

    /// PUT `uri` with no body, authenticated via external_id (used for reactions).
    pub async fn put(&self, uri: &str, external_id: &str) -> (StatusCode, serde_json::Value) {
        self.request("PUT", uri, Some(external_id), None).await
    }

    /// DELETE `uri` authenticated via external_id.
    pub async fn delete(&self, uri: &str, external_id: &str) -> (StatusCode, serde_json::Value) {
        self.request("DELETE", uri, Some(external_id), None).await
    }

    /// Send a request without authentication (no X-Auth-Consumer header).
    pub async fn post_unauthenticated(
        &self,
        uri: &str,
        body: serde_json::Value,
    ) -> (StatusCode, serde_json::Value) {
        self.request("POST", uri, None, Some(body)).await
    }

    /// Send a request without authentication, with any HTTP method and optional body.
    pub async fn request_no_auth(
        &self,
        method: &str,
        uri: &str,
        body: Option<serde_json::Value>,
    ) -> (StatusCode, serde_json::Value) {
        self.request(method, uri, None, body).await
    }

    /// Send a request with custom headers (claims, groups, etc.)
    pub async fn request_with_headers(
        &self,
        method: &str,
        uri: &str,
        external_id: &str,
        headers: &[(&str, &str)],
        body: Option<serde_json::Value>,
    ) -> (StatusCode, serde_json::Value) {
        let mut builder = Request::builder()
            .method(method)
            .uri(uri)
            .header("x-auth-consumer", external_id);
        for (k, v) in headers {
            builder = builder.header(*k, *v);
        }
        let req = if let Some(json) = body {
            builder = builder.header("Content-Type", "application/json");
            builder
                .body(Body::from(serde_json::to_vec(&json).unwrap()))
                .unwrap()
        } else {
            builder.body(Body::empty()).unwrap()
        };
        let response = self.router.clone().oneshot(req).await.unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
        (status, json)
    }

    async fn request(
        &self,
        method: &str,
        uri: &str,
        external_id: Option<&str>,
        body: Option<serde_json::Value>,
    ) -> (StatusCode, serde_json::Value) {
        let mut builder = Request::builder().method(method).uri(uri);

        if let Some(eid) = external_id {
            builder = builder.header("x-auth-consumer", eid);
        }

        let req = if let Some(json) = body {
            builder = builder.header("Content-Type", "application/json");
            builder
                .body(Body::from(serde_json::to_vec(&json).unwrap()))
                .unwrap()
        } else {
            builder.body(Body::empty()).unwrap()
        };

        let response = self.router.clone().oneshot(req).await.unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
        (status, json)
    }

    /// Send a request with a bearer token (no X-Auth-Consumer header).
    /// Used for incoming webhook trigger tests.
    pub async fn post_with_bearer(
        &self,
        uri: &str,
        token: &str,
        body: serde_json::Value,
    ) -> (StatusCode, serde_json::Value) {
        let req = Request::builder()
            .method("POST")
            .uri(uri)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {token}"))
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();
        let response = self.router.clone().oneshot(req).await.unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
        (status, json)
    }
}

// ── Seed helpers ──────────────────────────────────────────────────────────────

/// Create a minimal user row in the test database.
/// The `external_id` is set to the email so tests can authenticate
/// via the `X-Auth-Consumer` header using the same value.
pub async fn seed_user(pool: &PgPool, username: &str) -> db::users::UserRow {
    seed_user_with_role(pool, username, "member").await
}

/// Create a user with a specific role (e.g. "admin", "moderator", "member", "guest").
pub async fn seed_user_with_role(pool: &PgPool, username: &str, role: &str) -> db::users::UserRow {
    let email = format!("{username}@test.example");
    db::users::create(
        pool,
        &db::users::CreateUser {
            id: burst_core::id::new_id(),
            username: username.into(),
            display_name: username.into(),
            email: Some(email.clone()),
            password_hash: None,
            external_id: Some(email),
            role: role.into(),
        },
    )
    .await
    .unwrap()
}

/// Seed a root message directly into the database (bypasses the HTTP layer).
pub async fn seed_message(
    pool: &PgPool,
    channel_id: Uuid,
    author_id: Uuid,
    content: &str,
) -> db::messages::MessageRow {
    db::messages::create(
        pool,
        burst_core::id::new_id(),
        channel_id,
        author_id,
        None,
        content,
    )
    .await
    .unwrap()
}

/// Create a public channel owned by `owner_id` in the test database.
pub async fn seed_channel(pool: &PgPool, name: &str, owner_id: Uuid) -> db::channels::ChannelRow {
    let id = burst_core::id::new_id();
    let ch = db::channels::create(
        pool,
        &db::channels::CreateChannel {
            id,
            kind: "public".into(),
            name: Some(name.into()),
            slug: Some(name.to_lowercase().replace(' ', "-")),
            topic: None,
            description: None,
            created_by: owner_id,
        },
    )
    .await
    .unwrap();
    db::channels::add_member(pool, id, owner_id, "owner")
        .await
        .unwrap();
    ch
}

/// Create a webhook directly in the database. Returns the row and the plaintext token.
pub async fn seed_webhook(
    pool: &PgPool,
    channel_id: Uuid,
    kind: &str,
    created_by: Uuid,
) -> (db::webhooks::WebhookRow, String) {
    let token = format!("test_token_{}", burst_core::id::new_id());
    let row = db::webhooks::create(
        pool,
        &db::webhooks::CreateWebhook {
            id: burst_core::id::new_id(),
            channel_id,
            kind,
            name: &format!("test-{kind}-webhook"),
            url: if kind == "outgoing" {
                Some("https://example.com/hook")
            } else {
                None
            },
            secret: if kind == "outgoing" {
                Some("test_secret")
            } else {
                None
            },
            token: &token,
            created_by,
        },
    )
    .await
    .unwrap();
    (row, token)
}
