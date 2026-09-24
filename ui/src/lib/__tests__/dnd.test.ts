import { describe, it, expect } from "vitest";
import { applyDndToUserList, quietUntil, snoozeEnd } from "../dnd";
import type { User } from "../api/types";

function user(id: string, doNotDisturbUntil?: string): User {
  return {
    id,
    username: id,
    displayName: id,
    role: "member",
    status: "online",
    isBot: false,
    createdAt: "2026-01-01T00:00:00Z",
    doNotDisturbUntil,
  };
}

const NOW = Date.parse("2026-09-24T12:00:00Z");

describe("quietUntil", () => {
  it("is the end of the quiet period while it runs", () => {
    expect(quietUntil(user("a", "2026-09-24T13:00:00Z"), NOW)?.toISOString()).toBe("2026-09-24T13:00:00.000Z");
  });

  it("is undefined when not quiet or once the period has ended", () => {
    expect(quietUntil(user("a"), NOW)).toBeUndefined();
    expect(quietUntil(user("a", "2026-09-24T12:00:00Z"), NOW)).toBeUndefined();
    expect(quietUntil(undefined, NOW)).toBeUndefined();
  });
});

describe("applyDndToUserList", () => {
  it("updates that user only, and clears with no end", () => {
    const list = { items: [user("usr_a"), user("usr_b", "2026-09-24T18:00:00Z")] };
    const quiet = applyDndToUserList(list, { userId: "usr_a", until: "2026-09-24T13:00:00Z" })!;
    expect(quiet.items.map((u) => u.doNotDisturbUntil)).toEqual(["2026-09-24T13:00:00Z", "2026-09-24T18:00:00Z"]);
    const cleared = applyDndToUserList(quiet, { userId: "usr_b" })!;
    expect(cleared.items[1].doNotDisturbUntil).toBeUndefined();
  });
});

describe("snoozeEnd", () => {
  const afternoon = new Date(2026, 8, 24, 15, 30);

  it("adds a fixed duration", () => {
    expect(Date.parse(snoozeEnd("30m", afternoon)) - afternoon.getTime()).toBe(30 * 60_000);
    expect(Date.parse(snoozeEnd("2h", afternoon)) - afternoon.getTime()).toBe(2 * 3600_000);
  });

  it("runs until 09:00 local the next day", () => {
    expect(snoozeEnd("tomorrow", afternoon)).toBe(new Date(2026, 8, 25, 9).toISOString());
    expect(snoozeEnd("tomorrow", new Date(2026, 8, 30, 23, 50))).toBe(new Date(2026, 9, 1, 9).toISOString());
  });
});
