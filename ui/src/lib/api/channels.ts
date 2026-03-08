import { apiFetch } from "./client";
import type { Channel, Message, PaginatedResponse } from "./types";

export async function listChannels(): Promise<PaginatedResponse<Channel>> {
  return apiFetch<PaginatedResponse<Channel>>("/channels");
}

export async function getChannel(channelId: string): Promise<Channel> {
  return apiFetch<Channel>(`/channels/${channelId}`);
}

export async function createChannel(body: {
  name: string;
  slug?: string;
  kind?: string;
  topic?: string;
  description?: string;
}): Promise<Channel> {
  return apiFetch<Channel>("/channels", {
    method: "POST",
    body: JSON.stringify(body),
  });
}

export async function joinChannel(channelId: string): Promise<void> {
  await apiFetch<void>(`/channels/${channelId}/members`, {
    method: "POST",
  });
}

export async function listMessages(
  channelId: string,
  cursor?: string,
  limit = 50,
): Promise<PaginatedResponse<Message>> {
  const params = new URLSearchParams();
  if (cursor) params.set("cursor", cursor);
  params.set("limit", String(limit));
  const query = params.toString();
  return apiFetch<PaginatedResponse<Message>>(
    `/channels/${channelId}/messages${query ? `?${query}` : ""}`,
  );
}

export async function sendMessage(
  channelId: string,
  content: string,
): Promise<Message> {
  return apiFetch<Message>(`/channels/${channelId}/messages`, {
    method: "POST",
    body: JSON.stringify({ content }),
  });
}

export async function deleteMessage(
  channelId: string,
  messageId: string,
): Promise<void> {
  await apiFetch<void>(`/channels/${channelId}/messages/${messageId}`, {
    method: "DELETE",
  });
}
