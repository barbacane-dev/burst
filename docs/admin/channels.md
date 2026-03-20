## Channel Management

The admin panel gives you visibility into all channels, including private ones, and lets you archive or delete channels that are no longer needed.

## Viewing All Channels

From the admin panel at `/admin`, navigate to the **Channels** section. You will see every channel in the system, including:

- **Public channels** that anyone can join.
- **Private channels** that are normally hidden from non-members.
- **Archived channels** shown with a visual indicator.

For each channel, the list displays the name, type (public/private), member count, owner, and creation date.

## Channel Ownership

The user who creates a channel is its **owner**. Ownership grants:

- Full control over channel settings (name, topic, description).
- Ability to archive and unarchive the channel.
- Ability to manage the member list (invite, remove).

Admins can perform all owner actions on any channel, regardless of ownership.

## Archiving and Unarchiving

Archiving a channel puts it into **read-only mode**. Members can still access and read the full message history, but no new messages, reactions, or file uploads are allowed.

To archive a channel from the admin panel:

1. Open the admin panel at `/admin`.
2. Navigate to **Channels**.
3. Find the channel and open its action menu.
4. Select **Archive**.

To unarchive, follow the same steps and select **Unarchive**. The channel returns to full read-write mode immediately.

Archiving is useful when a project ends or a channel is no longer active but the conversation history should be preserved.

## Deleting Channels

Deleting a channel removes it and all its messages from the system. This action cannot be undone.

To delete a channel:

1. Open the admin panel at `/admin`.
2. Navigate to **Channels**.
3. Find the channel and open its action menu.
4. Select **Delete** and confirm the action.

Channel deletions are recorded in the [audit log](audit-log.md).

Only admins can delete channels. Channel owners who are not admins can archive but not delete.

## API

You can manage channels programmatically through the admin API. All admin endpoints require a valid JWT with the **admin** role.

### List All Channels

```
GET /admin/channels
```

Returns a paginated list of all channels, including private ones. Supports cursor-based pagination.

**Query parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `cursor` | string | UUIDv7 of the last item from the previous page. |
| `limit` | integer | Number of results per page (default: 50, max: 100). |

**Example response:**

```json
{
  "data": [
    {
      "id": "ch_018f3e...",
      "name": "engineering",
      "kind": "public",
      "archived": false,
      "memberCount": 24,
      "ownerId": "usr_018f3e...",
      "createdAt": "2026-01-10T08:00:00Z"
    }
  ],
  "pagination": {
    "nextCursor": "ch_018f3e..."
  }
}
```

### Delete a Channel

```
DELETE /admin/channels/{channelId}
```

Permanently deletes a channel and all associated messages, reactions, pins, and attachments.

Returns `204 No Content` on success.

**Example:**

```bash
curl -X DELETE https://gateway.example.com/admin/channels/ch_018f3e... \
  -H "Authorization: Bearer $TOKEN"
```

## See Also

- [Channels & Messaging](../guide/channels.md) for the end-user perspective.
- [Audit Log](audit-log.md) for tracking channel deletions and archival.
