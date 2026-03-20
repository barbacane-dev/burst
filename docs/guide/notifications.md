# Notifications

Burst uses browser notifications to alert you about new messages when the tab is not focused.

## Browser Notifications

When a new message arrives in a channel you're a member of and the Burst tab is not active, a browser notification appears with the message preview.

Your browser will prompt you to allow notifications the first time. You must grant permission for notifications to work.

## Per-Channel Preferences

You can control notifications for each channel independently. Open the channel settings and choose:

| Setting | Behavior |
|---------|----------|
| **All** (default) | Notified on every new message |
| **Mentions** | Notified only when someone `@mentions` you |
| **Nothing** | No notifications from this channel |

## @Mentions

Type `@` in the composer to see an autocomplete list of users. Selecting a user inserts an `@username` mention in your message.

When someone mentions you:

- The message is marked as a mention in the database.
- If your notification preference is set to "all" or "mentions", you receive a browser notification.

## Tips

- Set noisy channels to **Mentions** to reduce notification fatigue.
- Use **Nothing** for channels you read on your own schedule (e.g., announcements).
- Notifications only work over HTTPS (or localhost in development).
