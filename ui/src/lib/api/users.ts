import { apiFetch } from "./client";
import type { DoNotDisturb, DoNotDisturbSchedule, PaginatedResponse, User } from "./types";

export async function listUsers(cursor?: string): Promise<PaginatedResponse<User>> {
  const params = new URLSearchParams();
  if (cursor) params.set("cursor", cursor);
  const query = params.toString();
  return apiFetch<PaginatedResponse<User>>(`/users${query ? `?${query}` : ""}`);
}

export async function getMe(): Promise<User> {
  return apiFetch<User>("/users/me");
}

export async function updateMe(body: {
  displayName?: string;
  email?: string;
}): Promise<User> {
  return apiFetch<User>("/users/me", {
    method: "PATCH",
    body: JSON.stringify(body),
  });
}

export interface StatusInput {
  text?: string;
  emoji?: string;
  /** RFC 3339; omit for a status with no end. */
  expiresAt?: string;
}

export async function setMyStatus(status: StatusInput): Promise<User> {
  return apiFetch<User>("/users/me/status", {
    method: "PUT",
    body: JSON.stringify(status),
  });
}

export async function clearMyStatus(): Promise<User> {
  return apiFetch<User>("/users/me/status", { method: "DELETE" });
}
export async function getMyDoNotDisturb(): Promise<DoNotDisturb> {
  return apiFetch<DoNotDisturb>("/users/me/do-not-disturb");
}

/** Replaces the whole setting: an omitted snooze or schedule is cleared. */
export async function setMyDoNotDisturb(setting: {
  snoozeUntil?: string;
  schedule?: DoNotDisturbSchedule;
}): Promise<DoNotDisturb> {
  return apiFetch<DoNotDisturb>("/users/me/do-not-disturb", {
    method: "PUT",
    body: JSON.stringify(setting),
  });
}
