import type { PaginatedResponse, User } from "./api/types";

export interface ActiveStatus {
  text?: string;
  emoji?: string;
}

/** The status a user shows at `now`, or undefined when none is set or it has expired. */
export function activeStatus(
  user: Pick<User, "statusText" | "statusEmoji" | "statusExpiresAt"> | undefined,
  now: number = Date.now(),
): ActiveStatus | undefined {
  if (!user || (!user.statusText && !user.statusEmoji)) return undefined;
  if (user.statusExpiresAt && Date.parse(user.statusExpiresAt) <= now) return undefined;
  return { text: user.statusText, emoji: user.statusEmoji };
}

/** Payload of the `user.status_changed` event. All three fields are absent when cleared. */
export interface StatusChangedEvent {
  userId: string;
  text?: string;
  emoji?: string;
  expiresAt?: string;
}

export function withStatus<T extends User>(user: T, event: StatusChangedEvent): T {
  return {
    ...user,
    statusText: event.text,
    statusEmoji: event.emoji,
    statusExpiresAt: event.expiresAt,
  };
}

/** Applies a status change to the cached user list. */
export function applyToUserList(
  list: PaginatedResponse<User> | undefined,
  event: StatusChangedEvent,
): PaginatedResponse<User> | undefined {
  if (!list) return list;
  return {
    ...list,
    items: list.items.map((u) => (u.id === event.userId ? withStatus(u, event) : u)),
  };
}

export type ClearAfter = "never" | "30m" | "1h" | "4h" | "today" | "week";

export const CLEAR_AFTER_LABELS: Record<ClearAfter, string> = {
  never: "Don't clear",
  "30m": "30 minutes",
  "1h": "1 hour",
  "4h": "4 hours",
  today: "Today",
  week: "This week",
};

/**
 * When a status set at `now` with this choice expires. "Today" ends at the
 * next local midnight, "This week" at the next local Monday midnight.
 */
export function expiryFor(choice: ClearAfter, now: Date = new Date()): string | undefined {
  const minutes = { "30m": 30, "1h": 60, "4h": 240 } as const;
  if (choice === "never") return undefined;
  if (choice in minutes) {
    return new Date(now.getTime() + minutes[choice as keyof typeof minutes] * 60_000).toISOString();
  }
  const daysAhead = choice === "today" ? 1 : ((8 - now.getDay()) % 7 || 7);
  return new Date(now.getFullYear(), now.getMonth(), now.getDate() + daysAhead).toISOString();
}
