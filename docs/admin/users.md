## User Management

Burst has five roles that control what a user can do across the application.

## Roles

| Role | Permissions |
|------|-------------|
| **admin** | Full access. Manage users, channels, settings. Access the admin panel. |
| **integrator** | Everything a member can do, plus managing webhooks and bots. |
| **moderator** | Everything a member can do, plus moderating every channel they are a member of: deleting anyone's message, removing members, archiving. No reach into channels they are not in. |
| **member** | Send messages, create channels, browse and join public channels, add people to channels they are in. Default role for new users. |
| **guest** | Read and send only in channels someone added them to. Cannot browse, join, create channels or start a direct message, and cannot add people, pin, edit a channel's settings or moderate. |

A channel also has roles of its own: its **owner**, and any **moderators** the owner appoints. See [Channels](../guide/channels.md#moderators).

## Admin Panel

You can manage users from the admin panel at `/admin` in the Burst UI. The panel is only accessible to users with the **admin** role. From the users view, you can:

- See all registered users with their current role and status.
- Promote or demote users by changing their role.
- Deactivate users to revoke their access without deleting their data.

## Promoting and Demoting Users

To change a user's role:

1. Open the admin panel at `/admin`.
2. Navigate to the **Users** section.
3. Find the user and click their current role badge.
4. Select the new role from the dropdown.

Role changes take effect immediately. If the user is online, their session is updated in real time via WebSocket. Role changes are recorded in the [audit log](audit-log.md).

## JIT User Provisioning

Burst uses just-in-time (JIT) provisioning. When a user authenticates through OIDC for the first time, Burst automatically creates a local user record from the identity data provided by the gateway.

The flow works as follows:

1. The user authenticates with your identity provider (Keycloak, Auth0, Okta, etc.).
2. Barbacane validates the JWT and forwards the request with an `X-Auth-Consumer` header containing the `sub` claim.
3. Burst checks if a user with that subject identifier exists.
4. If not, Burst creates a new user with the **member** role and populates the profile from available JWT claims.

No manual user creation is needed. You only need to adjust roles after the user's first login if they need elevated permissions.

## API

You can also manage users programmatically through the admin API. All admin endpoints require a valid JWT with the **admin** role.

### List Users

```
GET /admin/users
```

Returns a paginated list of all users. Supports cursor-based pagination using UUIDv7 cursors.

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
      "id": "usr_018f3e...",
      "username": "alice",
      "displayName": "Alice Martin",
      "role": "admin",
      "status": "active",
      "createdAt": "2026-01-15T10:30:00Z"
    }
  ],
  "pagination": {
    "nextCursor": "usr_018f3e..."
  }
}
```

### Update a User

```
PATCH /admin/users/{userId}
```

Update a user's role or status.

**Request body:**

```json
{
  "role": "moderator"
}
```

**Possible values for `role`:** `admin`, `moderator`, `member`, `guest`.

The response returns the updated user object. A `403 Forbidden` is returned if you attempt to demote the last remaining admin.

## See Also

- [OIDC & SSO](oidc.md) for identity provider setup.
- [Audit Log](audit-log.md) for tracking role changes.
