## Audit Log

Burst records administrative actions in an audit log, giving you a tamper-evident trail of who did what and when.

## What Gets Logged

The audit log captures actions that change the security posture or structure of your workspace:

| Action | Description |
|--------|-------------|
| `user.role_changed` | A user's role was promoted or demoted. |
| `user.deactivated` | A user account was deactivated. |
| `user.reactivated` | A user account was reactivated. |
| `channel.archived` | A channel was put into read-only mode. |
| `channel.unarchived` | A channel was restored from read-only mode. |
| `channel.deleted` | A channel and its messages were permanently deleted. |
| `export.requested` | A [data export](data-export.md) was started. The metadata holds its scope and channel. |
| `export.downloaded` | A data export archive was downloaded. |
| `export.deleted` | A data export and its archive were deleted. |

Each entry records:

- **Timestamp** of the action.
- **Actor** -- the admin or moderator who performed it.
- **Target** -- the user or channel affected.
- **Action** -- the specific operation.
- **Details** -- contextual data such as the previous and new role.

## Viewing in the Admin Panel

You can browse the audit log from the admin panel:

1. Open the admin panel at `/admin`.
2. Navigate to the **Audit Log** section.
3. Entries are shown in reverse chronological order (newest first).

You can filter by target type (user or channel) to narrow down the results.

## API

### List Audit Log Entries

```
GET /admin/audit-log
```

Returns a paginated list of audit log entries. Requires a valid JWT with the **admin** role.

**Query parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `cursor` | string | UUIDv7 of the last entry from the previous page. |
| `limit` | integer | Number of results per page (default: 50, max: 100). |
| `targetType` | string | Filter by target type: `user` or `channel`. |

**Example request:**

```bash
curl "https://gateway.example.com/admin/audit-log?targetType=user&limit=10" \
  -H "Authorization: Bearer $TOKEN"
```

**Example response:**

```json
{
  "data": [
    {
      "id": "evt_018f3e...",
      "action": "user.role_changed",
      "actorId": "usr_018f3e...",
      "actorUsername": "alice",
      "targetId": "usr_018f4a...",
      "targetType": "user",
      "details": {
        "previousRole": "member",
        "newRole": "moderator"
      },
      "createdAt": "2026-03-15T14:22:00Z"
    }
  ],
  "pagination": {
    "nextCursor": "evt_018f3e..."
  }
}
```

## Pagination

The audit log uses cursor-based pagination with UUIDv7 cursors, consistent with all other list endpoints in Burst. Pass the `nextCursor` value from the response as the `cursor` parameter in the next request to fetch the following page.

When `nextCursor` is `null`, you have reached the end of the list.

## Retention

Audit log entries are never automatically deleted. They persist for the lifetime of the database. If you need to manage storage, consider archiving old entries to external storage through periodic database exports.

## See Also

- [User Management](users.md) for the actions that generate user audit entries.
- [Channel Management](channels.md) for the actions that generate channel audit entries.
