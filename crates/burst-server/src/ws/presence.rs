use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Tracks active WebSocket connection counts per user.
/// When a user's count goes from 0→1 they are online; from 1→0 they are offline.
pub struct PresenceState {
    connections: Arc<RwLock<HashMap<Uuid, u32>>>,
}

impl PresenceState {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Increment connection count. Returns true if this is the first connection (just came online).
    pub async fn connect(&self, user_id: Uuid) -> bool {
        let mut map = self.connections.write().await;
        let count = map.entry(user_id).or_insert(0);
        *count += 1;
        *count == 1
    }

    /// Decrement connection count. Returns true if this was the last connection (just went offline).
    pub async fn disconnect(&self, user_id: Uuid) -> bool {
        let mut map = self.connections.write().await;
        if let Some(count) = map.get_mut(&user_id) {
            if *count > 0 {
                *count -= 1;
            }
            if *count == 0 {
                map.remove(&user_id);
                return true;
            }
        }
        false
    }

    pub async fn is_online(&self, user_id: Uuid) -> bool {
        self.connections.read().await.contains_key(&user_id)
    }

    /// Returns the number of distinct online users.
    pub async fn online_count(&self) -> usize {
        self.connections.read().await.len()
    }
}
