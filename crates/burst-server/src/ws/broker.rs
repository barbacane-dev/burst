use std::sync::Arc;

use tokio::sync::broadcast;

use super::ServerEvent;

// ── EventBroker trait ───────────────────────────────────────────────────────

/// Abstraction over event fan-out, enabling single-node (in-process) and
/// multi-node (PG LISTEN/NOTIFY) implementations.
pub trait EventBroker: Send + Sync {
    /// Subscribe to receive events. Each subscriber gets its own receiver.
    fn subscribe(&self) -> broadcast::Receiver<ServerEvent>;
    /// Publish an event to all local subscribers (and remote nodes if applicable).
    fn publish(&self, event: ServerEvent);
}

pub type Broker = Arc<dyn EventBroker>;

// ── InProcessBroker (single-node, used in tests) ───────────────────────────

/// Broadcasts events to all subscribers via a tokio broadcast channel.
/// Suitable for single-node deployments and tests.
pub struct InProcessBroker {
    tx: broadcast::Sender<ServerEvent>,
}

impl InProcessBroker {
    pub fn new(capacity: usize) -> Arc<Self> {
        let (tx, _rx) = broadcast::channel(capacity);
        Arc::new(Self { tx })
    }
}

impl EventBroker for InProcessBroker {
    fn subscribe(&self) -> broadcast::Receiver<ServerEvent> {
        self.tx.subscribe()
    }

    fn publish(&self, event: ServerEvent) {
        let _ = self.tx.send(event);
    }
}

// ── PgNotifyBroker (multi-node via PostgreSQL LISTEN/NOTIFY) ────────────────

/// Wraps an InProcessBroker for local fan-out and uses PG LISTEN/NOTIFY
/// for cross-node synchronisation.
///
/// On publish: sends event locally AND issues `NOTIFY burst_events, '<payload>'`.
/// On startup: spawns a listener task that receives notifications from other
/// nodes and forwards them to the local broadcast channel.
pub struct PgNotifyBroker {
    local: Arc<InProcessBroker>,
    pool: sqlx::PgPool,
    node_id: String,
}

impl PgNotifyBroker {
    pub async fn new(
        pool: sqlx::PgPool,
        broadcast_capacity: usize,
    ) -> Result<Arc<Self>, sqlx::Error> {
        let local = InProcessBroker::new(broadcast_capacity);
        let node_id = uuid::Uuid::now_v7().to_string();
        let broker = Arc::new(Self {
            local: local.clone(),
            pool: pool.clone(),
            node_id,
        });

        // Spawn the listener task that receives notifications from other nodes.
        let listener_broker = broker.clone();
        tokio::spawn(async move {
            if let Err(e) = Self::listen_loop(listener_broker).await {
                tracing::error!(error = %e, "PG LISTEN loop terminated");
            }
        });

        Ok(broker)
    }

    async fn listen_loop(self: Arc<Self>) -> Result<(), sqlx::Error> {
        let mut listener = sqlx::postgres::PgListener::connect_with(&self.pool).await?;
        listener.listen("burst_events").await?;
        tracing::info!("PG LISTEN/NOTIFY broker started");

        loop {
            let notification = listener.recv().await?;
            let payload = notification.payload();

            // Parse the notification payload: { "node_id": "...", "event": {...} }
            let parsed: Result<NotifyPayload, _> = serde_json::from_str(payload);
            match parsed {
                Ok(notify) if notify.node_id != self.node_id => {
                    // Event from another node — publish locally.
                    self.local.publish(notify.event);
                }
                Ok(_) => {} // Our own event — already published locally, skip.
                Err(e) => {
                    tracing::warn!(error = %e, "failed to parse PG NOTIFY payload");
                }
            }
        }
    }
}

impl EventBroker for PgNotifyBroker {
    fn subscribe(&self) -> broadcast::Receiver<ServerEvent> {
        self.local.subscribe()
    }

    fn publish(&self, event: ServerEvent) {
        // Publish locally first (immediate delivery to this node's WS connections).
        self.local.publish(event.clone());

        // Publish to PG NOTIFY for other nodes (fire-and-forget).
        let pool = self.pool.clone();
        let payload = NotifyPayload {
            node_id: self.node_id.clone(),
            event,
        };
        tokio::spawn(async move {
            if let Ok(json) = serde_json::to_string(&payload)
                && let Err(e) = sqlx::query("SELECT pg_notify('burst_events', $1)")
                    .bind(&json)
                    .execute(&pool)
                    .await
            {
                tracing::warn!(error = %e, "failed to send PG NOTIFY");
            }
        });
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct NotifyPayload {
    node_id: String,
    event: ServerEvent,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn in_process_broker_delivers_events() {
        let broker = InProcessBroker::new(16);
        let mut rx = broker.subscribe();

        let event = ServerEvent::ChannelUpdated {
            event_id: "test".into(),
            channel_id: "ch_test".into(),
        };
        broker.publish(event.clone());

        let received = rx.recv().await.unwrap();
        assert_eq!(received.event_id(), "test");
    }

    #[tokio::test]
    async fn in_process_broker_multiple_subscribers() {
        let broker = InProcessBroker::new(16);
        let mut rx1 = broker.subscribe();
        let mut rx2 = broker.subscribe();

        let event = ServerEvent::ChannelUpdated {
            event_id: "multi".into(),
            channel_id: "ch_test".into(),
        };
        broker.publish(event);

        assert_eq!(rx1.recv().await.unwrap().event_id(), "multi");
        assert_eq!(rx2.recv().await.unwrap().event_id(), "multi");
    }
}
