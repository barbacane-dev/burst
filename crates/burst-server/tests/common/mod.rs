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

use burst_server::{AppState, app_router, auth::issue_access_token, config::AppConfig, db};

// ── TestApp ───────────────────────────────────────────────────────────────────

pub struct TestApp {
    /// The Axum router under test.  Clone it for each `oneshot` call.
    pub router: axum::Router,
    /// The full app state — useful for inspecting the broker, presence, etc.
    pub state: AppState,
}

impl TestApp {
    pub fn new(pool: PgPool) -> Self {
        let config = AppConfig {
            jwt_secret: "test-secret-key-that-is-at-least-32-chars".into(),
            jwt_expiry_seconds: 900,
            refresh_expiry_seconds: 3_600,
            trust_auth_headers: false,
            cookie_secure: false,
        };
        let state = AppState::new(pool, config);
        let router = app_router(state.clone());
        Self { router, state }
    }

    /// Issue a signed access token for `user_id` with the given role.
    pub fn token(&self, user_id: Uuid, role: &str) -> String {
        issue_access_token(&self.state.config, user_id, role).unwrap()
    }

    /// POST `uri` with a JSON body and a Bearer token.
    /// Returns `(status, json_body)`.
    pub async fn post(
        &self,
        uri: &str,
        token: &str,
        body: serde_json::Value,
    ) -> (StatusCode, serde_json::Value) {
        self.request("POST", uri, token, Some(body)).await
    }

    /// GET `uri` with a Bearer token.  Returns `(status, json_body)`.
    pub async fn get(&self, uri: &str, token: &str) -> (StatusCode, serde_json::Value) {
        self.request("GET", uri, token, None).await
    }

    /// PATCH `uri` with a JSON body and a Bearer token.
    pub async fn patch(
        &self,
        uri: &str,
        token: &str,
        body: serde_json::Value,
    ) -> (StatusCode, serde_json::Value) {
        self.request("PATCH", uri, token, Some(body)).await
    }

    /// PUT `uri` with no body and a Bearer token (used for reactions).
    pub async fn put(&self, uri: &str, token: &str) -> (StatusCode, serde_json::Value) {
        self.request("PUT", uri, token, None).await
    }

    /// DELETE `uri` with a Bearer token.
    pub async fn delete(&self, uri: &str, token: &str) -> (StatusCode, serde_json::Value) {
        self.request("DELETE", uri, token, None).await
    }

    async fn request(
        &self,
        method: &str,
        uri: &str,
        token: &str,
        body: Option<serde_json::Value>,
    ) -> (StatusCode, serde_json::Value) {
        let mut builder = Request::builder()
            .method(method)
            .uri(uri)
            .header("Authorization", format!("Bearer {token}"));

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
}

// ── Seed helpers ──────────────────────────────────────────────────────────────

/// Create a minimal user row in the test database.
pub async fn seed_user(pool: &PgPool, username: &str) -> db::users::UserRow {
    db::users::create(
        pool,
        &db::users::CreateUser {
            id: burst_core::id::new_id(),
            username: username.into(),
            display_name: username.into(),
            email: Some(format!("{username}@test.example")),
            password_hash: None,
            external_id: None,
            role: "member".into(),
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
