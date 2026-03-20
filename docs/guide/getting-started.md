# Getting Started

This guide walks you through signing in to Burst and sending your first message.

## Signing In

Burst uses your organization's identity provider (OIDC) for authentication, handled by the Barbacane gateway. When you open Burst, you'll be redirected to your SSO login page.

**Local development:** Sign in with `alice` or `bob` using any password. The mock OIDC server accepts any credentials for these users. Alice has the admin role; Bob is a regular member.

## The Interface

After signing in, you'll see:

- **Sidebar** (left) — Your channels and direct messages. Use the search icon or `Cmd+K` / `Ctrl+K` to search messages.
- **Message area** (center) — Messages in the selected channel, with a composer at the bottom.
- **Thread/Pins panel** (right) — Opens when you click a thread or the pin icon.

## Browsing and Joining Channels

Click the search icon next to "Channels" in the sidebar to browse all public channels. Click **Join** to become a member. Joined channels appear in your sidebar.

## Sending a Message

Select a channel from the sidebar, type your message in the composer at the bottom, and press **Enter** to send. Messages appear in real time for all channel members.

You can also:

- **Attach files** — Click the paperclip icon or drag and drop files into the composer.
- **Mention users** — Type `@` followed by a username for autocomplete.
- **Reply in a thread** — Click the reply icon on any message to open the thread panel.

## Creating a Channel

Click the **+** icon next to "Channels" in the sidebar. Enter a name and click **Create**. You'll be the channel owner and can set a topic or description.

## Starting a Direct Message

Click the **+** icon next to "Direct Messages" in the sidebar. Select a user from the list to start a conversation. If you've messaged them before, the existing DM opens.

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Cmd+K` / `Ctrl+K` | Open search |
| `Enter` | Send message |
| `Esc` | Close search / thread panel |
