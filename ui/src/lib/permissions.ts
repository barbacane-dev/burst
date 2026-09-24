/**
 * Which channel controls to show. A mirror of `burst_core::permissions`: the
 * server decides and refuses anything else, so this only keeps the interface
 * from offering what would fail. Keep the two in step; the tests here repeat
 * the Rust cases for that reason.
 */
import type { ChannelMember, User } from "./api/types";

export type InstanceRole = User["role"];
export type ChannelRole = ChannelMember["role"];

export interface Actor {
  instance: InstanceRole;
  /** `undefined` when not a member of the channel. */
  channel?: ChannelRole;
}

export type Action =
  | "createChannel"
  | "startDirectMessage"
  | "browsePublicChannels"
  | "joinPublicChannel"
  | "addMember"
  | "editChannel"
  | "pinMessage"
  | "deleteOthersMessage"
  | "archiveChannel";

const isGuest = (a: Actor) => a.instance === "guest";

/** Owner, channel moderator, or instance moderator who is a member; never a guest. */
export function moderates(a: Actor): boolean {
  if (isGuest(a)) return false;
  if (a.channel === "owner" || a.channel === "moderator") return true;
  return a.channel === "member" && a.instance === "moderator";
}

export function can(a: Actor, action: Action): boolean {
  if (a.instance === "admin") return true;
  switch (action) {
    case "createChannel":
    case "startDirectMessage":
    case "browsePublicChannels":
    case "joinPublicChannel":
      return !isGuest(a);
    case "addMember":
    case "editChannel":
    case "pinMessage":
      return a.channel !== undefined && !isGuest(a);
    case "deleteOthersMessage":
    case "archiveChannel":
      return moderates(a);
  }
}

function rankOf(role: ChannelRole | undefined): number {
  return role === "owner" ? 3 : role === "moderator" ? 2 : role === "member" ? 1 : 0;
}

function actorRank(a: Actor): number {
  return a.channel === "member" && moderates(a) ? rankOf("moderator") : rankOf(a.channel);
}

/** Nobody removes the owner; otherwise moderate and outrank the target. */
export function canRemove(a: Actor, target: ChannelRole): boolean {
  if (target === "owner") return false;
  if (a.instance === "admin") return true;
  return moderates(a) && actorRank(a) > rankOf(target);
}

/** Only the owner or an admin, and never to or from owner. */
export function canSetRole(a: Actor, current: ChannelRole, next: ChannelRole): boolean {
  if (current === "owner" || next === "owner") return false;
  return a.instance === "admin" || a.channel === "owner";
}
