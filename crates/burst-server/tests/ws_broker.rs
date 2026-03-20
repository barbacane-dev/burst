mod common;

use burst_server::ws::broker::{EventBroker, InProcessBroker, PgNotifyBroker};
use burst_server::ws::ServerEvent;

// ── InProcessBroker integration tests ────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn in_process_broker_delivers_to_subscriber(_pool: sqlx::PgPool) {
    let broker = InProcessBroker::new(64);
    let mut rx = broker.subscribe();

    let event = ServerEvent::MessageDeleted {
        event_id: "evt_1".into(),
        channel_id: "ch_test".into(),
        message_id: "msg_test".into(),
    };
    broker.publish(event);

    let received = rx.recv().await.unwrap();
    assert_eq!(received.event_id(), "evt_1");
}

#[sqlx::test(migrations = "../../migrations")]
async fn in_process_broker_fan_out_to_multiple_subscribers(_pool: sqlx::PgPool) {
    let broker = InProcessBroker::new(64);
    let mut rx1 = broker.subscribe();
    let mut rx2 = broker.subscribe();
    let mut rx3 = broker.subscribe();

    broker.publish(ServerEvent::ChannelUpdated {
        event_id: "evt_fan".into(),
        channel_id: "ch_1".into(),
    });

    assert_eq!(rx1.recv().await.unwrap().event_id(), "evt_fan");
    assert_eq!(rx2.recv().await.unwrap().event_id(), "evt_fan");
    assert_eq!(rx3.recv().await.unwrap().event_id(), "evt_fan");
}

#[sqlx::test(migrations = "../../migrations")]
async fn in_process_broker_subscriber_after_publish_misses_event(_pool: sqlx::PgPool) {
    let broker = InProcessBroker::new(64);
    broker.publish(ServerEvent::ChannelUpdated {
        event_id: "evt_old".into(),
        channel_id: "ch_1".into(),
    });

    // Subscriber created after publish should not see the old event.
    let mut rx = broker.subscribe();
    broker.publish(ServerEvent::ChannelUpdated {
        event_id: "evt_new".into(),
        channel_id: "ch_1".into(),
    });

    let received = rx.recv().await.unwrap();
    assert_eq!(received.event_id(), "evt_new");
}

// ── PgNotifyBroker integration tests ─────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn pg_notify_broker_delivers_locally(pool: sqlx::PgPool) {
    let broker = PgNotifyBroker::new(pool, 64).await.unwrap();
    let mut rx = broker.subscribe();

    broker.publish(ServerEvent::ChannelUpdated {
        event_id: "evt_pg_local".into(),
        channel_id: "ch_pg".into(),
    });

    let received = rx.recv().await.unwrap();
    assert_eq!(received.event_id(), "evt_pg_local");
}

#[sqlx::test(migrations = "../../migrations")]
async fn pg_notify_broker_serializes_events_to_pg(pool: sqlx::PgPool) {
    // Verify that the NOTIFY payload is sent correctly by creating a raw PG listener
    // and checking that it receives the notification from the broker.
    let broker = PgNotifyBroker::new(pool.clone(), 64).await.unwrap();

    let mut listener = sqlx::postgres::PgListener::connect_with(&pool)
        .await
        .unwrap();
    listener.listen("burst_events").await.unwrap();

    // Publish an event.
    broker.publish(ServerEvent::MessageDeleted {
        event_id: "evt_pg_notify".into(),
        channel_id: "ch_123".into(),
        message_id: "msg_456".into(),
    });

    // The notification is sent asynchronously via tokio::spawn, so give it a moment.
    let notification = tokio::time::timeout(std::time::Duration::from_secs(5), listener.recv())
        .await
        .expect("timed out waiting for PG notification")
        .expect("PG listener error");

    let payload = notification.payload();
    assert!(payload.contains("evt_pg_notify"), "payload should contain event_id");
    assert!(payload.contains("ch_123"), "payload should contain channel_id");
    assert!(payload.contains("msg_456"), "payload should contain message_id");
}

#[sqlx::test(migrations = "../../migrations")]
async fn pg_notify_broker_cross_node_delivery(pool: sqlx::PgPool) {
    // Simulate two nodes: broker A publishes, broker B should receive via PG NOTIFY.
    let broker_a = PgNotifyBroker::new(pool.clone(), 64).await.unwrap();
    let broker_b = PgNotifyBroker::new(pool.clone(), 64).await.unwrap();

    let mut rx_b = broker_b.subscribe();

    // Publish from broker A.
    broker_a.publish(ServerEvent::ChannelUpdated {
        event_id: "evt_cross_node".into(),
        channel_id: "ch_cross".into(),
    });

    // Broker B should receive the event via PG LISTEN/NOTIFY.
    let received = tokio::time::timeout(std::time::Duration::from_secs(5), rx_b.recv())
        .await
        .expect("timed out waiting for cross-node delivery")
        .expect("broadcast receive error");

    assert_eq!(received.event_id(), "evt_cross_node");
}

#[sqlx::test(migrations = "../../migrations")]
async fn pg_notify_broker_does_not_duplicate_own_events(pool: sqlx::PgPool) {
    // When broker A publishes, its own listener should ignore the notification
    // (dedup via node_id), so the subscriber should only see ONE event.
    let broker = PgNotifyBroker::new(pool, 64).await.unwrap();
    let mut rx = broker.subscribe();

    broker.publish(ServerEvent::ChannelUpdated {
        event_id: "evt_dedup".into(),
        channel_id: "ch_1".into(),
    });

    // Should receive exactly one event (the local publish).
    let received = rx.recv().await.unwrap();
    assert_eq!(received.event_id(), "evt_dedup");

    // Wait briefly — if dedup fails, a second copy would arrive from the listener.
    let result = tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await;
    assert!(result.is_err(), "should not receive a duplicate event");
}
