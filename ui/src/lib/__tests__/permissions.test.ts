import { describe, it, expect } from "vitest";
import { can, canRemove, canSetRole, moderates, type Action, type Actor } from "../permissions";

// The same cases as `burst_core::permissions`, so the controls shown match
// what the server allows.
const ALL: Action[] = [
  "createChannel",
  "startDirectMessage",
  "browsePublicChannels",
  "joinPublicChannel",
  "addMember",
  "editChannel",
  "pinMessage",
  "deleteOthersMessage",
  "archiveChannel",
];

const a = (instance: Actor["instance"], channel?: Actor["channel"]): Actor => ({ instance, channel });

describe("the interface offers what the server allows", () => {
  it("lets an admin do everything, even outside the channel", () => {
    for (const action of ALL) expect(can(a("admin"), action)).toBe(true);
  });

  it("lets a guest member do nothing beyond reading and sending", () => {
    for (const action of ALL) expect(can(a("guest", "member"), action)).toBe(false);
  });

  it("keeps what members could already do", () => {
    const member = a("member", "member");
    for (const action of ["createChannel", "addMember", "editChannel", "pinMessage"] as Action[]) {
      expect(can(member, action)).toBe(true);
    }
    expect(can(member, "deleteOthersMessage")).toBe(false);
  });

  it("needs membership for channel actions", () => {
    expect(can(a("member"), "addMember")).toBe(false);
    expect(can(a("member"), "pinMessage")).toBe(false);
  });

  it("lets an instance moderator moderate only channels they belong to", () => {
    expect(moderates(a("moderator", "member"))).toBe(true);
    expect(moderates(a("moderator"))).toBe(false);
    expect(canRemove(a("moderator", "member"), "member")).toBe(true);
    expect(canRemove(a("moderator", "member"), "moderator")).toBe(false);
  });

  it("never lets a guest moderate, whatever their channel role", () => {
    expect(moderates(a("guest", "moderator"))).toBe(false);
  });

  it("lets nobody remove the owner, and moderators not remove each other", () => {
    expect(canRemove(a("admin"), "owner")).toBe(false);
    expect(canRemove(a("member", "moderator"), "moderator")).toBe(false);
    expect(canRemove(a("member", "owner"), "moderator")).toBe(true);
  });

  it("lets only the owner or an admin appoint, and never to or from owner", () => {
    expect(canSetRole(a("member", "owner"), "member", "moderator")).toBe(true);
    expect(canSetRole(a("member", "moderator"), "member", "moderator")).toBe(false);
    expect(canSetRole(a("member", "owner"), "member", "owner")).toBe(false);
    expect(canSetRole(a("admin"), "owner", "member")).toBe(false);
  });
});
