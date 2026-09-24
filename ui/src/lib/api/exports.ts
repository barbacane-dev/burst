import { apiFetch, apiFetchBlob } from "./client";
import type { PaginatedResponse } from "./types";

export interface DataExport {
  id: string;
  scope: "instance" | "channel";
  channelId?: string;
  status: "pending" | "running" | "completed" | "failed";
  requestedBy: string;
  sizeBytes?: number;
  error?: string;
  createdAt: string;
  completedAt?: string;
}

export async function listExports(): Promise<PaginatedResponse<DataExport>> {
  return apiFetch<PaginatedResponse<DataExport>>("/admin/exports");
}

/** Exports one channel, or the whole instance when `channelId` is omitted. */
export async function createExport(channelId?: string): Promise<DataExport> {
  return apiFetch<DataExport>("/admin/exports", {
    method: "POST",
    body: JSON.stringify(channelId ? { channelId } : {}),
  });
}

export async function deleteExport(id: string): Promise<void> {
  return apiFetch<void>(`/admin/exports/${id}`, { method: "DELETE" });
}

/** Downloads a completed export and saves it through the browser. */
export async function downloadExport(id: string): Promise<void> {
  const { blob, fileName } = await apiFetchBlob(`/admin/exports/${id}/download`);
  const url = URL.createObjectURL(blob);
  try {
    const link = document.createElement("a");
    link.href = url;
    link.download = fileName ?? `${id}.zip`;
    link.click();
  } finally {
    URL.revokeObjectURL(url);
  }
}

export function isInProgress(e: DataExport): boolean {
  return e.status === "pending" || e.status === "running";
}

export function formatSize(bytes: number): string {
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${unit === 0 ? value : value.toFixed(1)} ${units[unit]}`;
}
