mod common;

use common::TestApp;
use serde_json::json;

/// JIT provisioning with OIDC claims extracts username, display name, email, and role.
#[sqlx::test]
async fn jit_provision_with_claims_and_groups(pool: sqlx::PgPool) {
    let app = TestApp::new(pool);

    let claims = json!({
        "sub": "a1b2c3d4-0000-0000-0000-000000000001",
        "name": "Alice Smith",
        "preferred_username": "alice",
        "email": "alice@example.com",
    });

    let (status, body) = app
        .request_with_headers(
            "GET",
            "/api/users/me",
            "a1b2c3d4-0000-0000-0000-000000000001",
            &[
                ("x-auth-claims", &serde_json::to_string(&claims).unwrap()),
                ("x-auth-consumer-groups", "admin"),
            ],
            None,
        )
        .await;

    assert_eq!(status, 200);
    assert_eq!(body["username"], "alice");
    assert_eq!(body["displayName"], "Alice Smith");
    assert_eq!(body["email"], "alice@example.com");
    assert_eq!(body["role"], "admin");
}

/// JIT provisioning without claims falls back to external_id for username.
#[sqlx::test]
async fn jit_provision_without_claims_uses_external_id(pool: sqlx::PgPool) {
    let app = TestApp::new(pool);

    let (status, body) = app.get("/api/users/me", "bob-external-id").await;

    assert_eq!(status, 200);
    assert_eq!(body["username"], "bob-external-id");
    assert_eq!(body["role"], "member");
}

/// Role is synced from groups header on subsequent requests.
#[sqlx::test]
async fn role_synced_from_groups_on_each_request(pool: sqlx::PgPool) {
    let app = TestApp::new(pool);
    let eid = "a1b2c3d4-0000-0000-0000-000000000002";

    let claims = json!({
        "sub": eid,
        "name": "Bob",
        "preferred_username": "bob",
    });
    let claims_str = serde_json::to_string(&claims).unwrap();

    // First request: member
    let (_, body) = app
        .request_with_headers(
            "GET",
            "/api/users/me",
            eid,
            &[
                ("x-auth-claims", &claims_str),
                ("x-auth-consumer-groups", "member"),
            ],
            None,
        )
        .await;
    assert_eq!(body["role"], "member");

    // Second request: promoted to admin
    let (_, body) = app
        .request_with_headers(
            "GET",
            "/api/users/me",
            eid,
            &[
                ("x-auth-claims", &claims_str),
                ("x-auth-consumer-groups", "admin"),
            ],
            None,
        )
        .await;
    assert_eq!(body["role"], "admin");
}
