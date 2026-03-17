use burst_server::ws::{EventBuffer, ServerEvent};
use uuid::Uuid;

fn make_presence_event(user_id: Uuid) -> (Uuid, ServerEvent) {
    let event_id = burst_core::id::new_id();
    (
        event_id,
        ServerEvent::PresenceUpdate {
            event_id: event_id.to_string(),
            user_id: burst_core::id::format_user_id(user_id),
            status: "online".into(),
        },
    )
}

// ── EventBuffer: basic push/retrieve ─────────────────────────────────────────

#[tokio::test]
async fn events_since_returns_subsequent_events() {
    let buffer = EventBuffer::new(100);
    let user = burst_core::id::new_id();

    let (id1, ev1) = make_presence_event(user);
    let (id2, ev2) = make_presence_event(user);
    let (id3, ev3) = make_presence_event(user);

    buffer.push(id1, ev1).await;
    buffer.push(id2, ev2).await;
    buffer.push(id3, ev3).await;

    let events = buffer.events_since(id1).await.unwrap();
    assert_eq!(events.len(), 2, "should return events after id1");
}

#[tokio::test]
async fn events_since_last_returns_empty() {
    let buffer = EventBuffer::new(100);
    let user = burst_core::id::new_id();

    let (id1, ev1) = make_presence_event(user);
    buffer.push(id1, ev1).await;

    let events = buffer.events_since(id1).await.unwrap();
    assert!(events.is_empty(), "no events after the last one");
}

#[tokio::test]
async fn events_since_unknown_id_returns_none() {
    let buffer = EventBuffer::new(100);
    let user = burst_core::id::new_id();

    let (id1, ev1) = make_presence_event(user);
    buffer.push(id1, ev1).await;

    let unknown_id = burst_core::id::new_id();
    let result = buffer.events_since(unknown_id).await;
    assert!(
        result.is_none(),
        "unknown event ID must return None (gap too large)"
    );
}

// ── EventBuffer: capacity ────────────────────────────────────────────────────

#[tokio::test]
async fn buffer_evicts_oldest_when_full() {
    let buffer = EventBuffer::new(3);
    let user = burst_core::id::new_id();

    let (id1, ev1) = make_presence_event(user);
    let (id2, ev2) = make_presence_event(user);
    let (id3, ev3) = make_presence_event(user);
    let (id4, ev4) = make_presence_event(user);

    buffer.push(id1, ev1).await;
    buffer.push(id2, ev2).await;
    buffer.push(id3, ev3).await;
    buffer.push(id4, ev4).await; // id1 should be evicted

    // id1 was evicted, so looking it up returns None
    assert!(buffer.events_since(id1).await.is_none());

    // id2 still exists, should return id3 and id4
    let events = buffer.events_since(id2).await.unwrap();
    assert_eq!(events.len(), 2);
}

#[tokio::test]
async fn empty_buffer_returns_none() {
    let buffer = EventBuffer::new(100);
    let unknown = burst_core::id::new_id();
    assert!(buffer.events_since(unknown).await.is_none());
}
