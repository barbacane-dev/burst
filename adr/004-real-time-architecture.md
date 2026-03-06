# ADR-004: Real-time Architecture

**Status:** Accepted
**Date:** 2026-03-06

## Context

Burst is a messaging tool — real-time delivery is not a nice-to-have, it is the core experience. Users expect messages to appear instantly, presence to update in real time, and typing indicators to feel responsive.

Per [ADR-003](003-technology-stack.md), we use WebSocket (tokio-tungstenite via Axum) for the real-time transport and PostgreSQL as the primary data store. We need to define:

1. How clients connect and authenticate over WebSocket.
2. How messages fan out from sender to recipients.
3. How this works across multiple server instances.
4. How we handle disconnections, reconnections, and message gaps.

Key constraint: the single-node deployment (one Burst binary + PostgreSQL) must work without any additional infrastructure. Multi-node support must be possible without re-architecting.

## Decision

### Connection lifecycle

1. **Connect:** Client opens a WebSocket connection to `/ws`. The connection is upgraded from an Axum HTTP route.
2. **Authenticate:** First message on the WebSocket must be an auth frame containing a valid session token (obtained via the REST API login flow, which goes through Barbacane). The server validates the token and associates the connection with a user. Unauthenticated connections are closed after a short timeout.
3. **Subscribe:** Upon authentication, the server automatically subscribes the connection to all channels and DMs the user is a member of. No explicit subscribe/unsubscribe per channel — membership is the subscription.
4. **Communicate:** Bidirectional JSON frames. Client sends actions (send message, typing indicator, presence update). Server pushes events (new message, message edited, user typing, presence change).
5. **Disconnect:** Server detects disconnection via WebSocket close or ping/pong timeout. Presence is updated after a grace period (to avoid flicker on brief network interruptions).

### Message fan-out (single node)

Messages flow through an in-process pub/sub layer:

```
Sender WS → Handler → Database (persist) → Broker → Recipient WS connections
                                                  ↘ Webhook dispatcher (async)
```

- **Broker:** An in-process channel-based pub/sub (tokio broadcast channels, one per channel/DM). Lightweight, zero-latency on a single node.
- **Persist first:** Messages are written to PostgreSQL before being broadcast. If the broadcast fails, the message is still durable and will be picked up on reconnection via gap-fill.
- **Fan-out:** The broker pushes the event to all WebSocket connections subscribed to that channel. Each connection has its own send task that serialises events to the wire.

### Multi-node synchronisation

When Burst runs as multiple instances behind a load balancer, in-process broadcast channels are not enough — a message sent to node A must reach users connected to node B.

**PostgreSQL LISTEN/NOTIFY as the sole broker.**

```
Node A: persist → PG NOTIFY 'channel:{id}' → Node B: PG LISTEN → local broadcast
```

- Each Burst instance maintains a persistent connection to PostgreSQL that LISTENs on relevant notification channels.
- When a message is persisted, the writing node issues a NOTIFY with a compact payload (event type + message ID). Receiving nodes fetch the full message from the database if needed.
- NOTIFY payloads are kept small (under 8KB PostgreSQL limit) — they carry just enough to identify the event, not the full message body.

This keeps the zero-extra-dependency promise. PG LISTEN/NOTIFY has limitations (no persistence of missed notifications, no backpressure, 8KB payload limit), but the gap-fill mechanism compensates — clients request missed events on reconnect regardless of why they were missed.

**Future broker options.** The broker interface is a trait, so alternative backends can be added later without re-architecting. RobustMQ (a Rust-native message broker supporting MQTT/Kafka/AMQP) is an interesting contender to watch once it reaches production readiness. NATS is another proven option, though it requires a Go server process.

### Reconnection and gap-fill

Clients will disconnect. Networks are unreliable. The protocol must handle this gracefully:

1. **Client-side reconnect:** Exponential backoff with jitter. The client stores the ID of the last event it received.
2. **Gap-fill on reconnect:** After re-authenticating, the client sends its last known event ID. The server queries PostgreSQL for all events after that ID for the user's subscribed channels and replays them before resuming live streaming.
3. **Bounded gap-fill:** If the gap is too large (e.g., client offline for days), the server sends a "full sync required" signal and the client falls back to REST API pagination.

### Event types

All events follow a consistent envelope:

```json
{
  "id": "evt_01H...",
  "type": "message.created",
  "channel_id": "ch_01H...",
  "timestamp": "2026-03-06T14:30:00Z",
  "payload": { ... }
}
```

Core event types:

| Type | Direction | Description |
|------|-----------|-------------|
| `message.created` | server → client | New message in a channel/DM |
| `message.updated` | server → client | Message edited |
| `message.deleted` | server → client | Message removed |
| `reaction.added` | server → client | Reaction added to a message |
| `reaction.removed` | server → client | Reaction removed |
| `typing.start` | both | User started typing (ephemeral, not persisted) |
| `typing.stop` | both | User stopped typing (ephemeral) |
| `presence.update` | server → client | User presence changed |
| `channel.updated` | server → client | Channel metadata changed |
| `member.joined` | server → client | User joined a channel |
| `member.left` | server → client | User left a channel |
| `send.message` | client → server | Client sends a message |
| `send.reaction` | client → server | Client adds/removes a reaction |

Ephemeral events (typing, presence) are broadcast but not persisted — they are not included in gap-fill.

### Presence

- Presence is tracked per-connection on each node.
- Each node maintains a local presence map (user → status).
- Presence changes are broadcast via the same broker (PG NOTIFY).
- A user is "online" if they have at least one active connection across any node.
- "Away" is set automatically after a configurable idle timeout (no client activity).
- A grace period (default: 30s) after the last connection drops before marking a user "offline" — this prevents flicker during page refreshes or brief network drops.

## Consequences

- **Persist-first** means no message loss even if the broadcast layer fails. The trade-off is a database write on the critical path of every message — acceptable at team scale, and PostgreSQL handles this comfortably.
- **PostgreSQL LISTEN/NOTIFY as the default multi-node broker** keeps the zero-extra-dependency promise. Missed notifications during node restarts are handled by client-side gap-fill, not by the broker itself.
- **Membership-as-subscription** simplifies the protocol (no subscribe/unsubscribe dance) but means the server must track channel membership changes and update subscriptions dynamically.
- **Gap-fill via event ID** requires monotonically ordered event IDs. We will use UUIDv7 (time-sortable) for event IDs, consistent with the rest of the data model.
- **Ephemeral events not persisted** keeps the database lean. The consequence is that typing indicators and presence are best-effort — they may be lost during reconnection, which is acceptable since they are transient by nature.
- **Broker trait abstraction** means we are not locked into PostgreSQL for pub/sub. If scale demands it, we can adopt RobustMQ, NATS, or another backend without changing the application layer.
