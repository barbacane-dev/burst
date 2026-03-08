import { apiFetch } from "./client";
import type { PaginatedResponse, User } from "./types";

export async function listUsers(cursor?: string): Promise<PaginatedResponse<User>> {
  const params = new URLSearchParams();
  if (cursor) params.set("cursor", cursor);
  const query = params.toString();
  return apiFetch<PaginatedResponse<User>>(`/users${query ? `?${query}` : ""}`);
}

export async function getMe(): Promise<User> {
  return apiFetch<User>("/users/me");
}
