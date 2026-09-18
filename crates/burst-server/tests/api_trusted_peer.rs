mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::{TestApp, with_peer};
use tower::ServiceExt;

/// Drive the router from a chosen peer, bypassing the helpers so the address is
/// the thing under test.
async fn get_as(app: &TestApp, peer: &str, external_id: &str) -> StatusCode {
    let req = with_peer(Request::builder(), peer)
        .method("GET")
        .uri("/api/users/me")
        .header("x-auth-consumer", external_id)
        .body(Body::empty())
        .expect("request");
    app.router
        .clone()
        .oneshot(req)
        .await
        .expect("response")
        .status()
}

/// The identity headers name the caller and carry the role, so a peer outside
/// `trusted_proxies` is anonymous however convincing its headers are. Without
/// this, anything able to reach Burst directly could claim any identity.
#[sqlx::test(migrations = "../../migrations")]
async fn identity_headers_are_refused_from_an_untrusted_peer(pool: sqlx::PgPool) {
    let app = TestApp::new(pool);
    let consumer = "a1b2c3d4-0000-0000-0000-000000000042";

    assert_eq!(
        get_as(&app, "127.0.0.1:54321", consumer).await,
        StatusCode::OK,
        "the trusted peer is the gateway and is believed"
    );

    assert_eq!(
        get_as(&app, "203.0.113.7:40000", consumer).await,
        StatusCode::UNAUTHORIZED,
        "the same headers from another peer must not authenticate"
    );
}

/// A forged identity from an untrusted peer must not create a user either, or
/// the check would be bypassed by the side effect.
#[sqlx::test(migrations = "../../migrations")]
async fn an_untrusted_peer_cannot_provision_a_user(pool: sqlx::PgPool) {
    let app = TestApp::new(pool);
    let forged = "a1b2c3d4-0000-0000-0000-0000000000ff";

    assert_eq!(
        get_as(&app, "198.51.100.9:40000", forged).await,
        StatusCode::UNAUTHORIZED
    );

    let found = burst_server::db::users::find_by_external_id(&app.state.db, forged)
        .await
        .expect("query");
    assert!(
        found.is_none(),
        "a refused request must not JIT-provision the identity it claimed"
    );
}

/// A request with no connection info recorded is refused rather than trusted,
/// so a server wired without it fails closed instead of silently accepting
/// every identity header.
#[sqlx::test(migrations = "../../migrations")]
async fn a_request_without_a_peer_address_is_refused(pool: sqlx::PgPool) {
    let app = TestApp::new(pool);
    let req = Request::builder()
        .method("GET")
        .uri("/api/users/me")
        .header("x-auth-consumer", "a1b2c3d4-0000-0000-0000-000000000001")
        .body(Body::empty())
        .expect("request");
    let status = app
        .router
        .clone()
        .oneshot(req)
        .await
        .expect("response")
        .status();
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
