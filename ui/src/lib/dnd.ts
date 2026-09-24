import type { PaginatedResponse, User, Weekday } from "./api/types";

export const WEEKDAYS: { day: Weekday; label: string }[] = [
  { day: "mon", label: "Mon" },
  { day: "tue", label: "Tue" },
  { day: "wed", label: "Wed" },
  { day: "thu", label: "Thu" },
  { day: "fri", label: "Fri" },
  { day: "sat", label: "Sat" },
  { day: "sun", label: "Sun" },
];

/** Payload of the `user.dnd_changed` event. */
export interface DndChangedEvent {
  userId: string;
  until?: string;
}

/** When `user`'s quiet period ends, or undefined if they are not quiet at `now`. */
export function quietUntil(
  user: Pick<User, "doNotDisturbUntil"> | undefined,
  now: number = Date.now(),
): Date | undefined {
  if (!user?.doNotDisturbUntil) return undefined;
  const until = new Date(user.doNotDisturbUntil);
  return until.getTime() > now ? until : undefined;
}

export function withDnd<T extends User>(user: T, event: DndChangedEvent): T {
  return { ...user, doNotDisturbUntil: event.until };
}

export function applyDndToUserList(
  list: PaginatedResponse<User> | undefined,
  event: DndChangedEvent,
): PaginatedResponse<User> | undefined {
  if (!list) return list;
  return {
    ...list,
    items: list.items.map((u) => (u.id === event.userId ? withDnd(u, event) : u)),
  };
}

export type SnoozeChoice = "30m" | "1h" | "2h" | "tomorrow";

export const SNOOZE_LABELS: Record<SnoozeChoice, string> = {
  "30m": "30 minutes",
  "1h": "1 hour",
  "2h": "2 hours",
  tomorrow: "Until tomorrow 9:00",
};

/** When a snooze started at `now` ends. "Tomorrow" is 09:00 local the next day. */
export function snoozeEnd(choice: SnoozeChoice, now: Date = new Date()): string {
  const minutes = { "30m": 30, "1h": 60, "2h": 120 } as const;
  if (choice === "tomorrow") {
    return new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1, 9).toISOString();
  }
  return new Date(now.getTime() + minutes[choice] * 60_000).toISOString();
}

/** The viewer's IANA time zone, for a new schedule. */
export function localTimeZone(): string {
  return Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC";
}
