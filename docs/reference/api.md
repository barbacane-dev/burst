## API Reference

Burst exposes a RESTful API for all messaging operations. The OpenAPI specification at `specs/burst-api.yaml` is the single source of truth for endpoints, request/response schemas, and Barbacane gateway configuration.

## Base URL

All API requests go through the Barbacane gateway. The base URL depends on your deployment:

```
https://gateway.example.com
```

Do not send requests directly to the Burst backend. The gateway handles authentication, rate limiting, and ACL enforcement before forwarding requests.

## Authentication

Every request must include a valid JWT in the `Authorization` header:

```
Authorization: Bearer <access_token>
```

The token is issued by your identity provider and validated by Barbacane's `oidc-auth` plugin. See [OIDC & SSO](../admin/oidc.md) for setup details.

## Pagination

All list endpoints use **cursor-based pagination** with UUIDv7 cursors. This provides stable results even when new items are created between page requests.

**Query parameters:**

| Parameter | Description |
|-----------|-------------|
| `cursor` | The `nextCursor` value from the previous response. Omit for the first page. |
| `limit` | Items per page. Defaults to 50, maximum 100. |

**Response envelope:**

```json
{
  "data": [ ... ],
  "pagination": {
    "nextCursor": "msg_018f3e..."
  }
}
```

When `nextCursor` is `null`, there are no more pages.

## Error Format

Burst returns errors as [RFC 9457 Problem Details](https://www.rfc-editor.org/rfc/rfc9457) objects:

```json
{
  "type": "urn:burst:error:not-found",
  "title": "Not Found",
  "status": 404,
  "detail": "Channel ch_018f3e... does not exist."
}
```

The `type` field is a stable URN you can match on programmatically. Common error types:

| URN | Status | Meaning |
|-----|--------|---------|
| `urn:burst:error:not-found` | 404 | The requested resource does not exist. |
| `urn:burst:error:validation` | 422 | The request body failed validation. |
| `urn:burst:error:forbidden` | 403 | You do not have permission for this action. |
| `urn:burst:error:conflict` | 409 | The resource already exists or the operation conflicts. |
| `urn:burst:error:rate-limited` | 429 | Too many requests. Retry after the indicated delay. |

## Endpoint Groups

The API is organized into the following groups. For full details on each endpoint, consult the OpenAPI spec.

### Channels

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/channels` | List channels you are a member of. |
| `POST` | `/channels` | Create a new channel. |
| `GET` | `/channels/{channelId}` | Get channel details. |
| `PATCH` | `/channels/{channelId}` | Update channel name, topic, or description. |
| `POST` | `/channels/{channelId}/join` | Join a public channel. |
| `POST` | `/channels/{channelId}/leave` | Leave a channel. |
| `GET` | `/channels/{channelId}/members` | List channel members. |

### Messages

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/channels/{channelId}/messages` | List messages in a channel. |
| `POST` | `/channels/{channelId}/messages` | Send a message. |
| `PATCH` | `/messages/{messageId}` | Edit a message. |
| `DELETE` | `/messages/{messageId}` | Delete a message (soft delete). |

### Threads

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/messages/{messageId}/replies` | List replies in a thread. |
| `POST` | `/messages/{messageId}/replies` | Reply to a message. |

### Reactions

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/messages/{messageId}/reactions` | Add a reaction. |
| `DELETE` | `/messages/{messageId}/reactions/{emoji}` | Remove a reaction. |

### Pins

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/channels/{channelId}/pins` | List pinned messages. |
| `POST` | `/messages/{messageId}/pin` | Pin a message. |
| `DELETE` | `/messages/{messageId}/pin` | Unpin a message. |

### Users

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/users/me` | Get your profile. |
| `PATCH` | `/users/me` | Update your display name or email. |
| `PUT` | `/users/me/status` | Set your custom status: text, emoji, optional expiry. |
| `DELETE` | `/users/me/status` | Clear your custom status. |
| `GET` | `/users/{userId}` | Get another user's profile. |

### Search

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/search/messages` | Full-text search across messages you have access to. |

### Files

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/channels/{channelId}/attachments` | Upload a file attachment. |
| `GET` | `/attachments/{attachmentId}` | Download a file. |

### Admin

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/admin/users` | List all users. |
| `PATCH` | `/admin/users/{userId}` | Update a user's role or status. |
| `GET` | `/admin/channels` | List all channels (including private). |
| `DELETE` | `/admin/channels/{channelId}` | Delete a channel. |
| `GET` | `/admin/audit-log` | List audit log entries. |

## OpenAPI Spec

The full specification is available at `specs/burst-api.yaml` in the repository. You can use tools like [Swagger UI](https://swagger.io/tools/swagger-ui/) or [Redoc](https://redocly.com/redoc) to render an interactive version.

## See Also

- [WebSocket Events](websocket.md) for real-time communication.
- [OIDC & SSO](../admin/oidc.md) for authentication setup.
