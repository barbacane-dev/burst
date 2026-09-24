import { apiFetch, apiFetchFormData } from "./client";
import type { Channel, ChannelMember, Message, PaginatedResponse } from "./types";

export async function listChannels(): Promise<PaginatedResponse<Channel>> {
  return apiFetch<PaginatedResponse<Channel>>("/channels?joined=true");
}

export async function browseChannels(): Promise<PaginatedResponse<Channel>> {
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

export async function createDm(userId: string): Promise<Channel> {
  return apiFetch<Channel>("/dms", {
    method: "POST",
    body: JSON.stringify({ userId }),
  });
}

export async function createGroupDm(userIds: string[]): Promise<Channel> {
  return apiFetch<Channel>("/dms", {
    method: "POST",
    body: JSON.stringify({ userIds }),
  });
}

export async function listMembers(channelId: string): Promise<ChannelMember[]> {
  const res = await apiFetch<PaginatedResponse<ChannelMember>>(`/channels/${channelId}/members`);
  return res.items;
}

export async function joinChannel(channelId: string): Promise<void> {
  await apiFetch<void>(`/channels/${channelId}/members`, {
    method: "POST",
  });
}

/** Adds someone to a channel. The caller must be a member and not a guest. */
export async function addMember(channelId: string, userId: string): Promise<void> {
  await apiFetch<void>(`/channels/${channelId}/members`, {
    method: "POST",
    body: JSON.stringify({ userId }),
  });
}

/** Removes another member. The caller must moderate the channel and outrank them. */
export async function removeMember(channelId: string, userId: string): Promise<void> {
  await apiFetch<void>(`/channels/${channelId}/members/${userId}`, { method: "DELETE" });
}

/** Appoints a channel moderator, or makes one a member again. Owner or admin only. */
export async function setMemberRole(
  channelId: string,
  userId: string,
  role: "moderator" | "member",
): Promise<ChannelMember> {
  return apiFetch<ChannelMember>(`/channels/${channelId}/members/${userId}`, {
    method: "PATCH",
    body: JSON.stringify({ role }),
  });
}

export async function leaveChannel(channelId: string): Promise<void> {
  await apiFetch<void>(`/channels/${channelId}/members/me`, { method: "DELETE" });
}

export async function archiveChannel(channelId: string): Promise<Channel> {
  return apiFetch<Channel>(`/channels/${channelId}/archive`, { method: "POST" });
}

export async function unarchiveChannel(channelId: string): Promise<Channel> {
  return apiFetch<Channel>(`/channels/${channelId}/unarchive`, { method: "POST" });
}

export async function markChannelRead(channelId: string): Promise<void> {
  await apiFetch<void>(`/channels/${channelId}/members/me/last-read`, {
    method: "PATCH",
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

export async function listThreadReplies(
  channelId: string,
  messageId: string,
  cursor?: string,
  limit = 50,
): Promise<PaginatedResponse<Message>> {
  const params = new URLSearchParams();
  if (cursor) params.set("cursor", cursor);
  params.set("limit", String(limit));
  const query = params.toString();
  return apiFetch<PaginatedResponse<Message>>(
    `/channels/${channelId}/messages/${messageId}/replies${query ? `?${query}` : ""}`,
  );
}

export async function sendMessage(
  channelId: string,
  content: string,
  threadId?: string,
  files?: File[],
): Promise<Message> {
  if (files && files.length > 0) {
    const formData = new FormData();
    formData.append("content", content);
    if (threadId) formData.append("threadId", threadId);
    for (const file of files) {
      formData.append("files", file, file.name);
    }
    return apiFetchFormData<Message>(`/channels/${channelId}/messages`, formData);
  }
  return apiFetch<Message>(`/channels/${channelId}/messages`, {
    method: "POST",
    body: JSON.stringify({ content, ...(threadId ? { threadId } : {}) }),
  });
}

export async function editMessage(
  channelId: string,
  messageId: string,
  content: string,
): Promise<Message> {
  return apiFetch<Message>(`/channels/${channelId}/messages/${messageId}`, {
    method: "PATCH",
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

export async function addReaction(
  channelId: string,
  messageId: string,
  emoji: string,
): Promise<void> {
  await apiFetch<void>(
    `/channels/${channelId}/messages/${messageId}/reactions/${encodeURIComponent(emoji)}`,
    { method: "PUT" },
  );
}

export async function removeReaction(
  channelId: string,
  messageId: string,
  emoji: string,
): Promise<void> {
  await apiFetch<void>(
    `/channels/${channelId}/messages/${messageId}/reactions/${encodeURIComponent(emoji)}`,
    { method: "DELETE" },
  );
}

export async function pinMessage(
  channelId: string,
  messageId: string,
): Promise<void> {
  await apiFetch<void>(
    `/channels/${channelId}/messages/${messageId}/pin`,
    { method: "PUT" },
  );
}

export async function unpinMessage(
  channelId: string,
  messageId: string,
): Promise<void> {
  await apiFetch<void>(
    `/channels/${channelId}/messages/${messageId}/pin`,
    { method: "DELETE" },
  );
}

export async function listPins(channelId: string): Promise<PaginatedResponse<Message>> {
  return apiFetch<PaginatedResponse<Message>>(`/channels/${channelId}/pins`);
}
