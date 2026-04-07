import { apiFetch } from "./client";
import type { PaginatedResponse } from "./types";

export interface Webhook {
  id: string;
  channelId: string;
  kind: "incoming" | "outgoing";
  name: string;
  url?: string;
  isActive: boolean;
  createdBy: string;
  createdAt: string;
}

export interface CreateWebhookResponse extends Webhook {
  token: string;
}

export async function listWebhooks(
  channelId: string,
  cursor?: string,
): Promise<PaginatedResponse<Webhook>> {
  const params = new URLSearchParams();
  if (cursor) params.set("cursor", cursor);
  const query = params.toString();
  return apiFetch<PaginatedResponse<Webhook>>(
    `/channels/${channelId}/webhooks${query ? `?${query}` : ""}`,
  );
}

export async function createWebhook(
  channelId: string,
  body: { kind: string; name: string; url?: string },
): Promise<CreateWebhookResponse> {
  return apiFetch<CreateWebhookResponse>(`/channels/${channelId}/webhooks`, {
    method: "POST",
    body: JSON.stringify(body),
  });
}

export async function updateWebhook(
  channelId: string,
  webhookId: string,
  body: { name?: string; url?: string; isActive?: boolean },
): Promise<Webhook> {
  return apiFetch<Webhook>(`/channels/${channelId}/webhooks/${webhookId}`, {
    method: "PATCH",
    body: JSON.stringify(body),
  });
}

export async function deleteWebhook(
  channelId: string,
  webhookId: string,
): Promise<void> {
  await apiFetch<void>(`/channels/${channelId}/webhooks/${webhookId}`, {
    method: "DELETE",
  });
}

export async function regenerateToken(
  channelId: string,
  webhookId: string,
): Promise<{ token: string }> {
  return apiFetch<{ token: string }>(
    `/channels/${channelId}/webhooks/${webhookId}/token`,
    { method: "POST" },
  );
}

export async function listAllWebhooks(
  cursor?: string,
): Promise<PaginatedResponse<Webhook>> {
  const params = new URLSearchParams();
  if (cursor) params.set("cursor", cursor);
  const query = params.toString();
  return apiFetch<PaginatedResponse<Webhook>>(
    `/admin/webhooks${query ? `?${query}` : ""}`,
  );
}
