import { apiFetch } from "./client";
import type { PaginatedResponse } from "./types";

export interface AdminUser {
  id: string;
  username: string;
  displayName: string;
  email?: string;
  role: string;
  isBot: boolean;
  deactivatedAt?: string;
  createdAt: string;
}

export interface AdminChannel {
  id: string;
  kind: string;
  name?: string;
  slug?: string;
  topic?: string;
  isArchived: boolean;
  isReadonly: boolean;
  createdAt: string;
}

export interface AuditLogEntry {
  id: string;
  userId?: string;
  action: string;
  targetType: string;
  targetId: string;
  metadata?: Record<string, unknown>;
  createdAt: string;
}

export async function listAdminUsers(
  cursor?: string,
): Promise<PaginatedResponse<AdminUser>> {
  const params = new URLSearchParams();
  if (cursor) params.set("cursor", cursor);
  const query = params.toString();
  return apiFetch<PaginatedResponse<AdminUser>>(`/admin/users${query ? `?${query}` : ""}`);
}

export async function updateAdminUser(
  userId: string,
  body: { role?: string; deactivated?: boolean },
): Promise<AdminUser> {
  return apiFetch<AdminUser>(`/admin/users/${userId}`, {
    method: "PATCH",
    body: JSON.stringify(body),
  });
}

export async function listAdminChannels(
  cursor?: string,
): Promise<PaginatedResponse<AdminChannel>> {
  const params = new URLSearchParams();
  if (cursor) params.set("cursor", cursor);
  const query = params.toString();
  return apiFetch<PaginatedResponse<AdminChannel>>(`/admin/channels${query ? `?${query}` : ""}`);
}

export async function updateAdminChannel(
  channelId: string,
  body: { isArchived?: boolean },
): Promise<AdminChannel> {
  return apiFetch<AdminChannel>(`/admin/channels/${channelId}`, {
    method: "PATCH",
    body: JSON.stringify(body),
  });
}

export async function deleteAdminChannel(channelId: string): Promise<void> {
  await apiFetch<void>(`/admin/channels/${channelId}`, {
    method: "DELETE",
  });
}

export async function listAuditLog(
  cursor?: string,
  targetType?: string,
): Promise<PaginatedResponse<AuditLogEntry>> {
  const params = new URLSearchParams();
  if (cursor) params.set("cursor", cursor);
  if (targetType) params.set("targetType", targetType);
  const query = params.toString();
  return apiFetch<PaginatedResponse<AuditLogEntry>>(`/admin/audit-log${query ? `?${query}` : ""}`);
}
