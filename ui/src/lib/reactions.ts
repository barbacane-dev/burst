import type { Message } from "./api/types";

export function addReactionLocally(msg: Message, emoji: string, userId: string): Message {
  const existing = msg.reactions.find((r) => r.emoji === emoji);
  if (existing) {
    if (existing.userIds.includes(userId)) return msg;
    return {
      ...msg,
      reactions: msg.reactions.map((r) =>
        r.emoji === emoji ? { ...r, count: r.count + 1, userIds: [...r.userIds, userId] } : r,
      ),
    };
  }
  return { ...msg, reactions: [...msg.reactions, { emoji, count: 1, userIds: [userId] }] };
}

export function removeReactionLocally(msg: Message, emoji: string, userId: string): Message {
  return {
    ...msg,
    reactions: msg.reactions
      .map((r) =>
        r.emoji === emoji
          ? { ...r, count: r.count - 1, userIds: r.userIds.filter((id) => id !== userId) }
          : r,
      )
      .filter((r) => r.count > 0),
  };
}
