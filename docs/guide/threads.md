# Threads & Replies

Threads keep side conversations organized without cluttering the main channel.

## Starting a Thread

Click the reply icon on any message. A thread panel opens on the right side of the screen, showing the original message and any replies.

## Replying

Type your reply in the thread panel's composer and press **Enter**. Thread replies appear below the original message in the panel.

In the main channel, the parent message shows a reply count badge (e.g., "3 replies") so others know a discussion is happening.

## Thread Model

Burst uses **flat threads** — replies are always direct children of the parent message. There are no nested sub-threads.

- A thread reply has a `threadId` pointing to the parent message.
- If the parent is itself a reply, the thread collapses to the root message.

## Closing the Thread Panel

Click the **X** in the thread panel header, or click a different message's reply icon to switch threads.

## Real-Time Updates

New replies appear in the thread panel in real time via WebSocket. If someone replies to a thread you're viewing, the message appears instantly.
