import { apiFetch } from "./client";
import type { PaginatedResponse } from "./types";
import type { AdminUser } from "./admin";

export async function listBots(): Promise<PaginatedResponse<AdminUser>> {
  return apiFetch<PaginatedResponse<AdminUser>>("/admin/bots");
}

export async function createBot(body: {
  username: string;
  displayName: string;
}): Promise<AdminUser> {
  return apiFetch<AdminUser>("/admin/bots", {
    method: "POST",
    body: JSON.stringify(body),
  });
}

export async function updateBot(
  botId: string,
  body: { displayName?: string },
): Promise<AdminUser> {
  return apiFetch<AdminUser>(`/admin/bots/${botId}`, {
    method: "PATCH",
    body: JSON.stringify(body),
  });
}

export async function deleteBot(botId: string): Promise<void> {
  await apiFetch<void>(`/admin/bots/${botId}`, {
    method: "DELETE",
  });
}
