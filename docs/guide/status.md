# Custom Status

A custom status tells others what you are up to: an emoji, a short text, or both. It shows beside your name on your messages and in a channel's member list, and updates live for everyone.

## Setting a Status

Open **Settings** and find the **Status** section.

- Pick a preset (In a meeting, Commuting, Working remotely, Out sick, On vacation), or
- Type an emoji and a status text of up to 100 characters.

Choose **Clear after** to have the status disappear on its own:

| Choice | Clears |
|--------|--------|
| Don't clear | Only when you clear it |
| 30 minutes, 1 hour, 4 hours | That long after you set it |
| Today | At midnight, in your time zone |
| This week | At midnight going into Monday, in your time zone |

Click **Set status**. Setting a new status replaces the previous one entirely.

## Clearing a Status

Click **Clear status** in the same section.

## Where It Shows

- **Messages**: the emoji follows your name. Hover it to read the text. A status with text only shows 💬.
- **Member list**: the emoji and text under your name.

## API

`PUT /api/users/me/status` with `{ "text", "emoji", "expiresAt" }` sets it, `DELETE /api/users/me/status` clears it. User objects carry `statusText`, `statusEmoji` and `statusExpiresAt`, and omit them once the status has expired. Changes are sent as `user.status_changed` WebSocket events.
