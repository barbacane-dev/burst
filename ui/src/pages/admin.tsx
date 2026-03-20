import { useState, useRef } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { Shield, Users, Hash, FileText, Smile } from "lucide-react";
import {
  listAdminUsers,
  listAdminChannels,
  listAuditLog,
  updateAdminUser,
  updateAdminChannel,
  deleteAdminChannel,
  type AdminUser,
  type AdminChannel,
  type AuditLogEntry,
} from "../lib/api/admin";
import {
  listAdminEmojis,
  createEmoji,
  deleteEmoji,
  type CustomEmoji,
} from "../lib/api/emojis";
import { Button } from "../components/ui/button";
import { Input } from "../components/ui/input";
import type { PaginatedResponse } from "../lib/api/types";

type Tab = "users" | "channels" | "emojis" | "audit";

export function AdminPage() {
  const [tab, setTab] = useState<Tab>("users");

  const tabs: { key: Tab; label: string; icon: React.ReactNode }[] = [
    { key: "users", label: "Users", icon: <Users className="h-4 w-4" /> },
    { key: "channels", label: "Channels", icon: <Hash className="h-4 w-4" /> },
    { key: "emojis", label: "Emojis", icon: <Smile className="h-4 w-4" /> },
    { key: "audit", label: "Audit Log", icon: <FileText className="h-4 w-4" /> },
  ];

  return (
    <div className="flex-1 overflow-y-auto">
      <div className="mx-auto max-w-4xl p-8">
        <div className="mb-6 flex items-center gap-3">
          <Shield className="h-6 w-6 text-indigo-600" />
          <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100">Administration</h1>
        </div>

        <div className="mb-6 flex gap-1 border-b border-gray-200 dark:border-gray-700">
          {tabs.map((t) => (
            <button
              key={t.key}
              onClick={() => setTab(t.key)}
              className={`flex items-center gap-2 border-b-2 px-4 py-2 text-sm font-medium transition-colors ${
                tab === t.key
                  ? "border-indigo-600 text-indigo-600 dark:text-indigo-400"
                  : "border-transparent text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-300"
              }`}
            >
              {t.icon}
              {t.label}
            </button>
          ))}
        </div>

        {tab === "users" && <UsersTab />}
        {tab === "channels" && <ChannelsTab />}
        {tab === "emojis" && <EmojisTab />}
        {tab === "audit" && <AuditTab />}
      </div>
    </div>
  );
}

function UsersTab() {
  const queryClient = useQueryClient();
  const { data, isLoading } = useQuery<PaginatedResponse<AdminUser>>({
    queryKey: ["admin-users"],
    queryFn: () => listAdminUsers(),
  });

  const mutation = useMutation({
    mutationFn: ({ userId, body }: { userId: string; body: { role?: string; deactivated?: boolean } }) =>
      updateAdminUser(userId, body),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["admin-users"] }),
  });

  const users = data?.items ?? [];

  return (
    <div className="space-y-3">
      {isLoading && <p className="text-sm text-gray-400">Loading users...</p>}
      {users.length === 0 && !isLoading && <p className="text-sm text-gray-400">No users found.</p>}
      {users.map((u) => (
        <div
          key={u.id}
          className="flex items-center justify-between rounded-lg border border-gray-200 bg-white p-4 dark:border-gray-700 dark:bg-gray-900"
        >
          <div>
            <div className="flex items-center gap-2">
              <p className="font-medium text-gray-900 dark:text-gray-100">{u.displayName}</p>
              <span className="text-xs text-gray-500">@{u.username}</span>
              {u.isBot && (
                <span className="rounded-full bg-blue-100 px-2 py-0.5 text-[10px] font-medium text-blue-700 dark:bg-blue-900/30 dark:text-blue-400">
                  Bot
                </span>
              )}
              {u.deactivatedAt && (
                <span className="rounded-full bg-red-100 px-2 py-0.5 text-[10px] font-medium text-red-700 dark:bg-red-900/30 dark:text-red-400">
                  Deactivated
                </span>
              )}
            </div>
            <p className="text-xs text-gray-500 dark:text-gray-400">
              {u.email ?? "No email"} &middot; {u.role} &middot; {new Date(u.createdAt).toLocaleDateString()}
            </p>
          </div>
          <div className="flex items-center gap-2">
            <select
              value={u.role}
              onChange={(e) =>
                mutation.mutate({ userId: u.id, body: { role: e.target.value } })
              }
              className="rounded-md border border-gray-300 bg-white px-2 py-1 text-xs dark:border-gray-600 dark:bg-gray-800 dark:text-gray-200"
            >
              <option value="admin">Admin</option>
              <option value="moderator">Moderator</option>
              <option value="member">Member</option>
              <option value="guest">Guest</option>
            </select>
            <Button
              variant="secondary"
              onClick={() =>
                mutation.mutate({
                  userId: u.id,
                  body: { deactivated: !u.deactivatedAt },
                })
              }
            >
              {u.deactivatedAt ? "Reactivate" : "Deactivate"}
            </Button>
          </div>
        </div>
      ))}
    </div>
  );
}

function ChannelsTab() {
  const queryClient = useQueryClient();
  const { data, isLoading } = useQuery<PaginatedResponse<AdminChannel>>({
    queryKey: ["admin-channels"],
    queryFn: () => listAdminChannels(),
  });

  const archiveMutation = useMutation({
    mutationFn: ({ channelId, isArchived }: { channelId: string; isArchived: boolean }) =>
      updateAdminChannel(channelId, { isArchived }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["admin-channels"] }),
  });

  const deleteMutation = useMutation({
    mutationFn: (channelId: string) => deleteAdminChannel(channelId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["admin-channels"] });
      queryClient.invalidateQueries({ queryKey: ["channels"] });
    },
  });

  const channels = data?.items ?? [];

  return (
    <div className="space-y-3">
      {isLoading && <p className="text-sm text-gray-400">Loading channels...</p>}
      {channels.length === 0 && !isLoading && <p className="text-sm text-gray-400">No channels found.</p>}
      {channels.map((ch) => (
        <div
          key={ch.id}
          className="flex items-center justify-between rounded-lg border border-gray-200 bg-white p-4 dark:border-gray-700 dark:bg-gray-900"
        >
          <div>
            <div className="flex items-center gap-2">
              <Hash className="h-4 w-4 text-gray-400" />
              <p className="font-medium text-gray-900 dark:text-gray-100">
                {ch.name ?? ch.slug ?? ch.id}
              </p>
              <span className="rounded-full bg-gray-100 px-2 py-0.5 text-[10px] font-medium text-gray-600 dark:bg-gray-800 dark:text-gray-400">
                {ch.kind}
              </span>
              {ch.isArchived && (
                <span className="rounded-full bg-yellow-100 px-2 py-0.5 text-[10px] font-medium text-yellow-700 dark:bg-yellow-900/30 dark:text-yellow-400">
                  Archived
                </span>
              )}
            </div>
            {ch.topic && (
              <p className="mt-0.5 text-xs text-gray-500 dark:text-gray-400">{ch.topic}</p>
            )}
          </div>
          <div className="flex items-center gap-2">
            <Button
              variant="secondary"
              onClick={() =>
                archiveMutation.mutate({
                  channelId: ch.id,
                  isArchived: !ch.isArchived,
                })
              }
            >
              {ch.isArchived ? "Unarchive" : "Archive"}
            </Button>
            <Button
              variant="secondary"
              onClick={() => {
                if (window.confirm(`Delete channel "${ch.name ?? ch.id}"? This cannot be undone.`)) {
                  deleteMutation.mutate(ch.id);
                }
              }}
            >
              Delete
            </Button>
          </div>
        </div>
      ))}
    </div>
  );
}

function AuditTab() {
  const { data, isLoading } = useQuery<PaginatedResponse<AuditLogEntry>>({
    queryKey: ["admin-audit"],
    queryFn: () => listAuditLog(),
  });

  const entries = data?.items ?? [];

  return (
    <div className="space-y-2">
      {isLoading && <p className="text-sm text-gray-400">Loading audit log...</p>}
      {entries.length === 0 && !isLoading && <p className="text-sm text-gray-400">No audit entries.</p>}
      {entries.map((e) => (
        <div
          key={e.id}
          className="flex items-start justify-between rounded-lg border border-gray-200 bg-white px-4 py-3 dark:border-gray-700 dark:bg-gray-900"
        >
          <div>
            <p className="text-sm font-medium text-gray-900 dark:text-gray-100">{e.action}</p>
            <p className="text-xs text-gray-500 dark:text-gray-400">
              {e.targetType}/{e.targetId}
              {e.userId && ` by ${e.userId}`}
            </p>
          </div>
          <time className="text-xs text-gray-400 dark:text-gray-500">
            {new Date(e.createdAt).toLocaleString()}
          </time>
        </div>
      ))}
    </div>
  );
}

// ── Emojis Tab ──

function EmojisTab() {
  const queryClient = useQueryClient();
  const [shortcode, setShortcode] = useState("");
  const [error, setError] = useState("");
  const fileRef = useRef<HTMLInputElement>(null);

  const { data, isLoading } = useQuery<PaginatedResponse<CustomEmoji>>({
    queryKey: ["admin-emojis"],
    queryFn: () => listAdminEmojis(),
  });

  const uploadMutation = useMutation({
    mutationFn: ({ sc, file }: { sc: string; file: File }) => createEmoji(sc, file),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["admin-emojis"] });
      queryClient.invalidateQueries({ queryKey: ["emojis"] });
      setShortcode("");
      setError("");
      if (fileRef.current) fileRef.current.value = "";
    },
    onError: (err: Error) => setError(err.message),
  });

  const deleteMutation = useMutation({
    mutationFn: (id: string) => deleteEmoji(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["admin-emojis"] });
      queryClient.invalidateQueries({ queryKey: ["emojis"] });
    },
  });

  function handleUpload() {
    const file = fileRef.current?.files?.[0];
    if (!file || !shortcode.trim()) return;
    setError("");
    uploadMutation.mutate({ sc: shortcode.trim(), file });
  }

  const emojis = data?.items ?? [];

  return (
    <div className="space-y-4">
      <div className="rounded-lg border border-gray-200 bg-white p-4 dark:border-gray-700 dark:bg-gray-900">
        <h3 className="mb-3 text-sm font-semibold text-gray-900 dark:text-gray-100">
          Upload Custom Emoji
        </h3>
        {error && (
          <div className="mb-3 rounded-md bg-red-50 p-3 text-sm text-red-700 dark:bg-red-900/30 dark:text-red-400">
            {error}
          </div>
        )}
        <div className="flex items-end gap-3">
          <Input
            id="shortcode"
            label="Shortcode"
            value={shortcode}
            onChange={(e) => setShortcode(e.target.value)}
            placeholder="partyparrot"
          />
          <div>
            <label className="mb-1 block text-sm font-medium text-gray-700 dark:text-gray-300">
              Image
            </label>
            <input
              ref={fileRef}
              type="file"
              accept="image/*"
              className="text-sm text-gray-500 file:mr-2 file:rounded file:border-0 file:bg-indigo-50 file:px-3 file:py-1.5 file:text-sm file:font-medium file:text-indigo-600 dark:text-gray-400 dark:file:bg-indigo-900/30 dark:file:text-indigo-400"
            />
          </div>
          <Button onClick={handleUpload} disabled={uploadMutation.isPending}>
            Upload
          </Button>
        </div>
      </div>

      {isLoading && <p className="text-sm text-gray-400">Loading emojis...</p>}

      {emojis.length === 0 && !isLoading && (
        <p className="text-sm text-gray-400">No custom emojis yet.</p>
      )}

      <div className="grid grid-cols-2 gap-2 sm:grid-cols-3 md:grid-cols-4">
        {emojis.map((emoji) => (
          <div
            key={emoji.id}
            className="flex items-center gap-2 rounded-lg border border-gray-200 bg-white px-3 py-2 dark:border-gray-700 dark:bg-gray-900"
          >
            <img
              src={`/api/attachments/${emoji.imageUrl}`}
              alt={emoji.shortcode}
              className="h-6 w-6 object-contain"
            />
            <span className="flex-1 truncate text-sm text-gray-900 dark:text-gray-100">
              :{emoji.shortcode}:
            </span>
            <button
              onClick={() => deleteMutation.mutate(emoji.id)}
              disabled={deleteMutation.isPending}
              className="text-xs text-red-500 hover:text-red-700 disabled:opacity-50"
            >
              Delete
            </button>
          </div>
        ))}
      </div>
    </div>
  );
}
