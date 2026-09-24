import { describe, it, expect } from "vitest";
import { activeStatus, applyToUserList, expiryFor } from "../status";
import type { User } from "../api/types";

const NOW = Date.parse("2026-09-24T12:00:00Z");

function user(id: string, extra: Partial<User> = {}): User {
  return {
    id,
    username: id,
    displayName: id,
    role: "member",
    status: "online",
    isBot: false,
    createdAt: "2026-01-01T00:00:00Z",
    ...extra,
  };
}

describe("activeStatus", () => {
  it("is undefined without a text or an emoji", () => {
    expect(activeStatus(user("a"), NOW)).toBeUndefined();
    expect(activeStatus(undefined, NOW)).toBeUndefined();
  });

  it("shows either part alone", () => {
    expect(activeStatus(user("a", { statusText: "Lunch" }), NOW)).toEqual({ text: "Lunch" });
    expect(activeStatus(user("a", { statusEmoji: "🍕" }), NOW)).toEqual({ emoji: "🍕" });
  });

  it("stops at the expiry", () => {
    const status = { statusText: "Meeting", statusExpiresAt: "2026-09-24T12:00:01Z" };
    expect(activeStatus(user("a", status), NOW)).toEqual({ text: "Meeting" });
    expect(activeStatus(user("a", status), NOW + 1000)).toBeUndefined();
  });
});

describe("applyToUserList", () => {
  it("replaces the status of that user only", () => {
    const list = {
      items: [user("usr_a", { statusText: "Old", statusEmoji: "🐢" }), user("usr_b", { statusText: "Keep" })],
    };
    const next = applyToUserList(list, { userId: "usr_a", text: "New" })!;
    expect(next.items[0]).toMatchObject({ statusText: "New", statusEmoji: undefined });
    expect(next.items[1].statusText).toBe("Keep");
  });

  it("clears when the event carries no status", () => {
    const list = { items: [user("usr_a", { statusText: "Old", statusExpiresAt: "2027-01-01T00:00:00Z" })] };
    const next = applyToUserList(list, { userId: "usr_a" })!;
    expect(activeStatus(next.items[0], NOW)).toBeUndefined();
    expect(next.items[0].statusExpiresAt).toBeUndefined();
  });
});

describe("expiryFor", () => {
  // Wednesday 24 September 2026, 15:30 local.
  const wednesday = new Date(2026, 8, 24, 15, 30);

  it("has no end for never", () => {
    expect(expiryFor("never", wednesday)).toBeUndefined();
  });

  it("adds a fixed duration", () => {
    expect(Date.parse(expiryFor("30m", wednesday)!) - wednesday.getTime()).toBe(30 * 60_000);
    expect(Date.parse(expiryFor("4h", wednesday)!) - wednesday.getTime()).toBe(4 * 3600_000);
  });

  it("ends today at the next local midnight", () => {
    expect(expiryFor("today", wednesday)).toBe(new Date(2026, 8, 25).toISOString());
  });

  it("ends this week at the next local Monday midnight", () => {
    expect(expiryFor("week", wednesday)).toBe(new Date(2026, 8, 28).toISOString());
    // On a Sunday the week ends that night; on a Monday, a week later.
    expect(expiryFor("week", new Date(2026, 8, 27, 10))).toBe(new Date(2026, 8, 28).toISOString());
    expect(expiryFor("week", new Date(2026, 8, 28, 10))).toBe(new Date(2026, 9, 5).toISOString());
  });
});
