# Notifications

Burst uses browser notifications to alert you about new messages when the tab is not focused.

## Browser Notifications

When a message arrives that your preference for its channel covers, and the Burst tab is not active, a browser notification appears naming the author and the channel, with a preview of the message. Clicking it opens the channel.

You are never notified of your own messages.

Your browser will prompt you to allow notifications the first time. You must grant permission for notifications to work.

## Per-Channel Preferences

You can control notifications for each channel independently. Open the channel settings and choose:

| Setting | Behavior |
|---------|----------|
| **All** (default) | Notified on every new message |
| **Mentions** | Notified only when someone `@mentions` you |
| **Nothing** | No notifications from this channel |

## @Mentions

Type `@` in the composer to see an autocomplete list of users. Selecting a user inserts an `@username` mention in your message. Mentions are highlighted in the conversation.

When someone mentions you, you are notified if your preference for the channel is **All** or **Mentions**. **Nothing** stays silent, mentions included.

## @channel and @here

Two mentions address more than one person. Both appear at the top of the autocomplete list.

| Mention | Reaches |
|---------|---------|
| `@channel` | Every member of the channel |
| `@here` | The members who are online when the message is sent |

Each counts as a mention for every member it reaches, so it notifies members whose preference is **Mentions**, and does not notify members whose preference is **Nothing**. The author is never included.

A member who is offline is mentioned by `@channel` and not by `@here`. Use `@here` for something that only matters to whoever is around right now.

Because `channel` and `here` always mean these two, a user with either as their username cannot be mentioned by name.

## Tips

- Set noisy channels to **Mentions** to reduce notification fatigue. You will still hear `@channel`, and `@here` while you are online.
- Use **Nothing** for channels you read on your own schedule (e.g., announcements).
- Notifications only work over HTTPS (or localhost in development).
