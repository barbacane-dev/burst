## WebSocket Events

Burst uses a WebSocket connection for real-time messaging. All events flow through a single connection per client.

## Connecting

Open a WebSocket connection to the gateway:

```
ws://gateway.example.com/ws?access_token=<JWT>
```

The `access_token` query parameter carries your JWT. Barbacane validates the token before upgrading the connection. If the token is invalid or expired, the connection is rejected with a `401` close frame.

For production deployments behind TLS, use `wss://` instead of `ws://`.

## Event Envelope

All messages, both from the server and client, use the same JSON envelope:

```json
{
  "id": "evt_018f3e...",
  "type": "message.created",
  "payload": { ... },
  "timestamp": "2026-03-15T14:22:00Z"
}
```

| Field | Description |
|-------|-------------|
| `id` | UUIDv7 event identifier. Monotonically increasing. |
| `type` | Event type string (see tables below). |
| `payload` | Event-specific data. |
| `timestamp` | ISO 8601 timestamp of when the event was created. |

## Server Events

These events are sent from the server to your client.

### Messages

| Type | Payload | Description |
|------|---------|-------------|
| `message.created` | Full message object | A new message was sent in a channel you are a member of. |
| `message.updated` | Full message object | A message was edited. |
| `message.deleted` | `{ messageId, channelId }` | A message was deleted. |

### Reactions

| Type | Payload | Description |
|------|---------|-------------|
| `reaction.added` | `{ messageId, userId, emoji }` | A reaction was added to a message. |
| `reaction.removed` | `{ messageId, userId, emoji }` | A reaction was removed from a message. |

### Typing Indicators

| Type | Payload | Description |
|------|---------|-------------|
| `typing.start` | `{ channelId, userId, username }` | A user started typing in a channel. |
| `typing.stop` | `{ channelId, userId }` | A user stopped typing. |

### Presence

| Type | Payload | Description |
|------|---------|-------------|
| `presence.update` | `{ userId, status }` | A user's presence changed (`online`, `away`, `offline`). |
| `user.status_changed` | `{ userId, text?, emoji?, expiresAt? }` | A user set or cleared their custom status. All three fields are absent when it was cleared. Sent to every connected user. |

### Channels

| Type | Payload | Description |
|------|---------|-------------|
| `channel.joined` | Full channel object | You were added to a channel (or joined one). |
| `channel.updated` | Full channel object | A channel's name, topic, or settings changed. |

### Pins

| Type | Payload | Description |
|------|---------|-------------|
| `message.pinned` | `{ messageId, channelId, pinnedBy }` | A message was pinned. |
| `message.unpinned` | `{ messageId, channelId }` | A message was unpinned. |

### Notifications

| Type | Payload | Description |
|------|---------|-------------|
| `notification.created` | `{ notificationId, recipientId, channelId, messageId, reason, authorName, channelName?, preview }` | The server decided to notify you of a message. |

Every member of a channel receives `message.created`, including its author. Use `notification.created` to decide whether to alert a user, not `message.created`.

The server sends a notification only to the member it is addressed to, and decides who that is from each member's preference for the channel:

| Preference | Notified of |
|------------|-------------|
| `all` (default) | Every message. `reason` is `mention` when the message mentions you, otherwise `message`. |
| `mentions` | Only messages that mention you. |
| `nothing` | Nothing, mentions included. |

A message mentions you when it names you, when it contains `@channel`, or when it contains `@here` and you are online as it is sent.

You are never notified of your own messages. `channelName` is absent for direct messages. `preview` holds up to 200 characters of the message and is empty when the message carries only files.

## Client Events

You can send these events from your client to the server.

| Type | Payload | Description |
|------|---------|-------------|
| `heartbeat` | `{}` | Keep the connection alive. Send every 30 seconds. |
| `typing.start` | `{ channelId }` | Notify that you started typing. |
| `typing.stop` | `{ channelId }` | Notify that you stopped typing. |

The server does not acknowledge client events. If the connection is alive, events are processed immediately.

### Heartbeat

Send a `heartbeat` event at least every 30 seconds to prevent the server from closing your connection. The server considers a connection dead after 60 seconds of inactivity.

```json
{
  "type": "heartbeat",
  "payload": {}
}
```

## Gap-Fill on Reconnect

If your client disconnects and reconnects, you may have missed events. To catch up, include the `lastEventId` query parameter when reconnecting:

```
ws://gateway.example.com/ws?access_token=<JWT>&lastEventId=evt_018f3e...
```

The server replays all events that occurred after the given event ID. This works because event IDs are UUIDv7 values, which are both unique and chronologically ordered.

If too many events have accumulated (more than the server's replay buffer), the server sends a `sync.required` event, indicating that your client should re-fetch state from the REST API.

Notifications are not replayed: one missed while disconnected is stale by the time you reconnect, and the message itself is. They carry a `notificationId` rather than an event ID, so do not use one as `lastEventId`.

## Connection Lifecycle

1. **Connect** with `access_token` (and optionally `lastEventId`).
2. **Receive** a `presence.update` event confirming your online status.
3. **Send** `heartbeat` events every 30 seconds.
4. **Receive** server events as they occur.
5. **Send** `typing.start`/`typing.stop` when composing messages.
6. On disconnect, **reconnect** with `lastEventId` set to the last event you received.

## Error Handling

If the server needs to close the connection, it sends a WebSocket close frame with a reason:

| Code | Reason |
|------|--------|
| `1000` | Normal closure (server shutting down gracefully). |
| `1008` | Policy violation (invalid or expired token). |
| `4000` | Rate limited (too many messages sent). |

On receiving a `1008` close, refresh your access token before reconnecting.

## See Also

- [API Reference](api.md) for REST endpoints.
- [Getting Started](../guide/getting-started.md) for the user-facing overview.
