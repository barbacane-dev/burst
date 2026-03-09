/// Unit tests for `PresenceState`.
///
/// These are pure in-memory tests — no database, no HTTP.
/// They run as fast as regular `#[test]` cases even though they use `async`.
use burst_server::ws::presence::PresenceState;
use uuid::Uuid;

fn uid() -> Uuid {
    burst_core::id::new_id()
}

// ── connect ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn first_connect_returns_true() {
    let p = PresenceState::new();
    assert!(
        p.connect(uid()).await,
        "first connect must signal came-online"
    );
}

#[tokio::test]
async fn second_connect_returns_false() {
    let p = PresenceState::new();
    let u = uid();
    p.connect(u).await;
    assert!(
        !p.connect(u).await,
        "second connect must not signal came-online again"
    );
}

#[tokio::test]
async fn multiple_users_are_independent() {
    let p = PresenceState::new();
    let (a, b) = (uid(), uid());
    assert!(p.connect(a).await);
    assert!(
        p.connect(b).await,
        "second *user* must also signal came-online"
    );
}

// ── disconnect ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn last_disconnect_returns_true() {
    let p = PresenceState::new();
    let u = uid();
    p.connect(u).await;
    assert!(
        p.disconnect(u).await,
        "disconnecting the last session must signal went-offline"
    );
}

#[tokio::test]
async fn non_last_disconnect_returns_false() {
    let p = PresenceState::new();
    let u = uid();
    p.connect(u).await;
    p.connect(u).await; // second tab
    assert!(
        !p.disconnect(u).await,
        "disconnecting with one session still open must not signal went-offline"
    );
}

#[tokio::test]
async fn disconnect_unknown_user_returns_false() {
    let p = PresenceState::new();
    assert!(
        !p.disconnect(uid()).await,
        "disconnecting a user who never connected must return false"
    );
}

// ── is_online ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn is_online_after_connect() {
    let p = PresenceState::new();
    let u = uid();
    p.connect(u).await;
    assert!(p.is_online(u).await);
}

#[tokio::test]
async fn is_offline_before_any_connect() {
    let p = PresenceState::new();
    assert!(!p.is_online(uid()).await);
}

#[tokio::test]
async fn is_offline_after_all_sessions_close() {
    let p = PresenceState::new();
    let u = uid();
    p.connect(u).await;
    p.connect(u).await;
    p.disconnect(u).await;
    p.disconnect(u).await;
    assert!(
        !p.is_online(u).await,
        "user must be offline after all sessions disconnect"
    );
}

/// Closing one tab must not put the user offline if they still have another open.
#[tokio::test]
async fn still_online_after_partial_disconnect() {
    let p = PresenceState::new();
    let u = uid();
    p.connect(u).await;
    p.connect(u).await;
    p.disconnect(u).await;
    assert!(
        p.is_online(u).await,
        "user must remain online with one session left"
    );
}
