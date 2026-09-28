import { useState, useRef, useMemo } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { Shield, Users, Hash, FileText, Smile, Webhook, Bot, Archive } from "lucide-react";
import { useAuth } from "../lib/auth/use-auth";
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
  emojiImageSrc,
  listAdminEmojis,
  createEmoji,
  deleteEmoji,
  type CustomEmoji,
} from "../lib/api/emojis";
import {
  listAllWebhooks,
  createWebhook,
  deleteWebhook,
  type Webhook as WebhookType,
  type CreateWebhookResponse,
} from "../lib/api/webhooks";
import { listChannels } from "../lib/api/channels";
import {
  listBots,
  createBot,
  deleteBot,
} from "../lib/api/bots";
import {
  createExport,
  deleteExport,
  downloadExport,
  formatSize,
  isInProgress,
  listExports,
  type DataExport,
} from "../lib/api/exports";
import { Button } from "../components/ui/button";
import { Input } from "../components/ui/input";
import type { PaginatedResponse } from "../lib/api/types";

type Tab = "users" | "channels" | "webhooks" | "bots" | "emojis" | "exports" | "audit";

const allTabs: { key: Tab; label: string; icon: React.ReactNode; adminOnly: boolean }[] = [
  { key: "users", label: "Users", icon: <Users className="h-4 w-4" />, adminOnly: true },
  { key: "channels", label: "Channels", icon: <Hash className="h-4 w-4" />, adminOnly: true },
  { key: "webhooks", label: "Webhooks", icon: <Webhook className="h-4 w-4" />, adminOnly: false },
  { key: "bots", label: "Bots", icon: <Bot className="h-4 w-4" />, adminOnly: false },
  { key: "emojis", label: "Emojis", icon: <Smile className="h-4 w-4" />, adminOnly: true },
  { key: "exports", label: "Exports", icon: <Archive className="h-4 w-4" />, adminOnly: true },
  { key: "audit", label: "Audit Log", icon: <FileText className="h-4 w-4" />, adminOnly: true },
];

export function AdminPage() {
  const { user } = useAuth();
  const isAdmin = user?.role === "admin";
  const tabs = useMemo(
    () => allTabs.filter((t) => !t.adminOnly || isAdmin),
    [isAdmin],
  );
  const [tab, setTab] = useState<Tab>(isAdmin ? "users" : "webhooks");

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
        {tab === "webhooks" && <WebhooksTab />}
        {tab === "bots" && <BotsTab />}
        {tab === "emojis" && <EmojisTab />}
        {tab === "exports" && <ExportsTab />}
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
              <option value="integrator">Integrator</option>
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

// ── Exports Tab ──

const STATUS_LABEL: Record<DataExport["status"], string> = {
  pending: "Queued",
  running: "Building",
  completed: "Ready",
  failed: "Failed",
};

export function ExportsTab() {
  const queryClient = useQueryClient();
  const [channelId, setChannelId] = useState("");
  const [downloadError, setDownloadError] = useState<string>();

  const { data, isLoading } = useQuery<PaginatedResponse<DataExport>>({
    queryKey: ["admin-exports"],
    queryFn: listExports,
    // Follows an export while it is being built.
    refetchInterval: (query) => (query.state.data?.items.some(isInProgress) ? 2000 : false),
  });
  const { data: channels } = useQuery<PaginatedResponse<AdminChannel>>({
    queryKey: ["admin-channels"],
    queryFn: () => listAdminChannels(),
  });

  const create = useMutation({
    mutationFn: () => createExport(channelId || undefined),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["admin-exports"] }),
  });
  const remove = useMutation({
    mutationFn: (id: string) => deleteExport(id),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["admin-exports"] }),
  });

  const exports = data?.items ?? [];
  const busy = exports.some(isInProgress);
  const channelName = (id?: string) => {
    const c = channels?.items.find((ch) => ch.id === id);
    return c?.name ? `#${c.name}` : "a deleted channel";
  };

  async function download(id: string) {
    setDownloadError(undefined);
    try {
      await downloadExport(id);
    } catch (e) {
      setDownloadError(e instanceof Error ? e.message : "Download failed");
    }
  }

  return (
    <div className="space-y-4">
      <div className="rounded-lg border border-gray-200 bg-white p-4 dark:border-gray-700 dark:bg-gray-900">
        <p className="mb-3 text-sm text-gray-600 dark:text-gray-300">
          An export is a zip of messages and files, for compliance or to move elsewhere. The whole
          instance includes private channels and direct messages. Deleted messages are left out.
          Requests and downloads are recorded in the audit log.
        </p>
        <div className="flex flex-wrap items-center gap-2">
          <select
            aria-label="What to export"
            value={channelId}
            onChange={(e) => setChannelId(e.target.value)}
            className="rounded-md border border-gray-300 bg-white px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
          >
            <option value="">Whole instance</option>
            {(channels?.items ?? [])
              .filter((c) => c.name)
              .map((c) => (
                <option key={c.id} value={c.id}>
                  #{c.name}
                </option>
              ))}
          </select>
          <Button onClick={() => create.mutate()} disabled={busy || create.isPending}>
            {busy ? "Export in progress" : "Start export"}
          </Button>
          {create.isError && (
            <span className="text-sm text-red-600 dark:text-red-400">{create.error.message}</span>
          )}
        </div>
      </div>

      {downloadError && <p className="text-sm text-red-600 dark:text-red-400">{downloadError}</p>}
      {isLoading && <p className="text-sm text-gray-400">Loading exports...</p>}
      {exports.length === 0 && !isLoading && <p className="text-sm text-gray-400">No exports yet.</p>}
      <ul className="space-y-2" aria-label="Exports">
        {exports.map((e) => (
          <li
            key={e.id}
            className="flex items-center justify-between gap-4 rounded-lg border border-gray-200 bg-white px-4 py-3 dark:border-gray-700 dark:bg-gray-900"
          >
            <div className="min-w-0">
              <p className="text-sm font-medium text-gray-900 dark:text-gray-100">
                {e.scope === "instance" ? "Whole instance" : channelName(e.channelId)}
                <span className="ml-2 rounded bg-gray-100 px-1.5 py-0.5 text-xs text-gray-600 dark:bg-gray-800 dark:text-gray-300">
                  {STATUS_LABEL[e.status]}
                </span>
              </p>
              <p className="text-xs text-gray-500 dark:text-gray-400">
                {new Date(e.createdAt).toLocaleString()}
                {e.sizeBytes !== undefined && ` · ${formatSize(e.sizeBytes)}`}
                {e.error && ` · ${e.error}`}
              </p>
            </div>
            <div className="flex shrink-0 gap-2">
              {e.status === "completed" && (
                <Button variant="secondary" onClick={() => download(e.id)}>
                  Download
                </Button>
              )}
              {!isInProgress(e) && (
                <Button variant="secondary" onClick={() => remove.mutate(e.id)} disabled={remove.isPending}>
                  Delete
                </Button>
              )}
            </div>
          </li>
        ))}
      </ul>
    </div>
  );
}

// ── Webhooks Tab ──

function WebhooksTab() {
  const queryClient = useQueryClient();
  const [showCreate, setShowCreate] = useState(false);
  const [newToken, setNewToken] = useState<string | null>(null);
  const [form, setForm] = useState({ kind: "incoming", name: "", url: "", channelId: "" });
  const [error, setError] = useState("");

  const { data: webhooksData, isLoading } = useQuery<PaginatedResponse<WebhookType>>({
    queryKey: ["admin-webhooks"],
    queryFn: () => listAllWebhooks(),
  });

  const { data: channelsData } = useQuery({
    queryKey: ["channels"],
    queryFn: () => listChannels(),
  });

  const createMutation = useMutation({
    mutationFn: () =>
      createWebhook(form.channelId, {
        kind: form.kind,
        name: form.name,
        url: form.kind === "outgoing" ? form.url : undefined,
      }),
    onSuccess: (data: CreateWebhookResponse) => {
      queryClient.invalidateQueries({ queryKey: ["admin-webhooks"] });
      setNewToken(data.token);
      setForm({ kind: "incoming", name: "", url: "", channelId: "" });
      setShowCreate(false);
      setError("");
    },
    onError: (err: Error) => setError(err.message),
  });

  const deleteMutation = useMutation({
    mutationFn: ({ channelId, webhookId }: { channelId: string; webhookId: string }) =>
      deleteWebhook(channelId, webhookId),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["admin-webhooks"] }),
  });

  const webhooks = webhooksData?.items ?? [];
  const channels = channelsData?.items ?? [];

  return (
    <div className="space-y-4">
      {newToken && (
        <div className="rounded-lg border border-green-200 bg-green-50 p-4 dark:border-green-800 dark:bg-green-900/20">
          <p className="mb-2 text-sm font-medium text-green-800 dark:text-green-300">
            Webhook token (copy now — it will not be shown again):
          </p>
          <code className="block break-all rounded bg-green-100 px-3 py-2 text-xs text-green-900 dark:bg-green-900/40 dark:text-green-200">
            {newToken}
          </code>
          <Button variant="secondary" onClick={() => { navigator.clipboard.writeText(newToken); }} className="mt-2">
            Copy
          </Button>
          <Button variant="secondary" onClick={() => setNewToken(null)} className="ml-2 mt-2">
            Dismiss
          </Button>
        </div>
      )}

      {!showCreate ? (
        <Button onClick={() => setShowCreate(true)}>Create Webhook</Button>
      ) : (
        <div className="rounded-lg border border-gray-200 bg-white p-4 dark:border-gray-700 dark:bg-gray-900">
          <h3 className="mb-3 text-sm font-semibold text-gray-900 dark:text-gray-100">
            New Webhook
          </h3>
          {error && (
            <div className="mb-3 rounded-md bg-red-50 p-3 text-sm text-red-700 dark:bg-red-900/30 dark:text-red-400">
              {error}
            </div>
          )}
          <div className="flex flex-col gap-3">
            <div className="flex gap-3">
              <select
                value={form.channelId}
                onChange={(e) => setForm({ ...form, channelId: e.target.value })}
                className="flex-1 rounded-md border border-gray-300 bg-white px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-200"
              >
                <option value="">Select channel...</option>
                {channels.map((ch) => (
                  <option key={ch.id} value={ch.id}>
                    {ch.name ?? ch.slug ?? ch.id}
                  </option>
                ))}
              </select>
              <select
                value={form.kind}
                onChange={(e) => setForm({ ...form, kind: e.target.value })}
                className="rounded-md border border-gray-300 bg-white px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-200"
              >
                <option value="incoming">Incoming</option>
                <option value="outgoing">Outgoing</option>
              </select>
            </div>
            <Input
              label="Name"
              value={form.name}
              onChange={(e) => setForm({ ...form, name: e.target.value })}
              placeholder="CI Notifications"
            />
            {form.kind === "outgoing" && (
              <Input
                label="URL"
                value={form.url}
                onChange={(e) => setForm({ ...form, url: e.target.value })}
                placeholder="https://example.com/webhook"
              />
            )}
            <div className="flex gap-2">
              <Button
                onClick={() => createMutation.mutate()}
                disabled={!form.channelId || !form.name || createMutation.isPending}
              >
                Create
              </Button>
              <Button variant="secondary" onClick={() => { setShowCreate(false); setError(""); }}>
                Cancel
              </Button>
            </div>
          </div>
        </div>
      )}

      {isLoading && <p className="text-sm text-gray-400">Loading webhooks...</p>}
      {webhooks.length === 0 && !isLoading && <p className="text-sm text-gray-400">No webhooks configured.</p>}
      {webhooks.map((wh) => (
        <div
          key={wh.id}
          className="flex items-center justify-between rounded-lg border border-gray-200 bg-white p-4 dark:border-gray-700 dark:bg-gray-900"
        >
          <div>
            <div className="flex items-center gap-2">
              <p className="font-medium text-gray-900 dark:text-gray-100">{wh.name}</p>
              <span className={`rounded-full px-2 py-0.5 text-[10px] font-medium ${
                wh.kind === "incoming"
                  ? "bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400"
                  : "bg-purple-100 text-purple-700 dark:bg-purple-900/30 dark:text-purple-400"
              }`}>
                {wh.kind}
              </span>
              {!wh.isActive && (
                <span className="rounded-full bg-gray-100 px-2 py-0.5 text-[10px] font-medium text-gray-600 dark:bg-gray-800 dark:text-gray-400">
                  Inactive
                </span>
              )}
            </div>
            <p className="text-xs text-gray-500 dark:text-gray-400">
              Channel: {wh.channelId}
              {wh.url && <> &middot; {wh.url}</>}
            </p>
            {wh.kind === "incoming" && (
              <p className="mt-1 text-xs text-gray-400">
                Trigger: POST /api/webhooks/{wh.id}/trigger
              </p>
            )}
          </div>
          <Button
            variant="secondary"
            onClick={() => {
              if (window.confirm(`Delete webhook "${wh.name}"?`)) {
                deleteMutation.mutate({ channelId: wh.channelId, webhookId: wh.id });
              }
            }}
          >
            Delete
          </Button>
        </div>
      ))}
    </div>
  );
}

// ── Bots Tab ──

function BotsTab() {
  const queryClient = useQueryClient();
  const [showCreate, setShowCreate] = useState(false);
  const [form, setForm] = useState({ username: "", displayName: "" });
  const [error, setError] = useState("");

  const { data, isLoading } = useQuery<PaginatedResponse<AdminUser>>({
    queryKey: ["admin-bots"],
    queryFn: () => listBots(),
  });

  const createMutation = useMutation({
    mutationFn: () => createBot({ username: form.username, displayName: form.displayName }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["admin-bots"] });
      queryClient.invalidateQueries({ queryKey: ["admin-users"] });
      setForm({ username: "", displayName: "" });
      setShowCreate(false);
      setError("");
    },
    onError: (err: Error) => setError(err.message),
  });

  const deleteMutation = useMutation({
    mutationFn: (botId: string) => deleteBot(botId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["admin-bots"] });
      queryClient.invalidateQueries({ queryKey: ["admin-users"] });
    },
  });

  const bots = data?.items ?? [];

  return (
    <div className="space-y-4">
      {!showCreate ? (
        <Button onClick={() => setShowCreate(true)}>Create Bot</Button>
      ) : (
        <div className="rounded-lg border border-gray-200 bg-white p-4 dark:border-gray-700 dark:bg-gray-900">
          <h3 className="mb-3 text-sm font-semibold text-gray-900 dark:text-gray-100">
            New Bot
          </h3>
          {error && (
            <div className="mb-3 rounded-md bg-red-50 p-3 text-sm text-red-700 dark:bg-red-900/30 dark:text-red-400">
              {error}
            </div>
          )}
          <div className="flex flex-col gap-3">
            <Input
              label="Username"
              value={form.username}
              onChange={(e) => setForm({ ...form, username: e.target.value })}
              placeholder="ci-bot"
            />
            <Input
              label="Display Name"
              value={form.displayName}
              onChange={(e) => setForm({ ...form, displayName: e.target.value })}
              placeholder="CI Bot"
            />
            <p className="text-xs text-gray-500 dark:text-gray-400">
              Bot credentials are managed in the Barbacane gateway (apikey-auth plugin).
            </p>
            <div className="flex gap-2">
              <Button
                onClick={() => createMutation.mutate()}
                disabled={!form.username || !form.displayName || createMutation.isPending}
              >
                Create
              </Button>
              <Button variant="secondary" onClick={() => { setShowCreate(false); setError(""); }}>
                Cancel
              </Button>
            </div>
          </div>
        </div>
      )}

      {isLoading && <p className="text-sm text-gray-400">Loading bots...</p>}
      {bots.length === 0 && !isLoading && <p className="text-sm text-gray-400">No bots configured.</p>}
      {bots.map((bot) => (
        <div
          key={bot.id}
          className="flex items-center justify-between rounded-lg border border-gray-200 bg-white p-4 dark:border-gray-700 dark:bg-gray-900"
        >
          <div>
            <div className="flex items-center gap-2">
              <p className="font-medium text-gray-900 dark:text-gray-100">{bot.displayName}</p>
              <span className="text-xs text-gray-500">@{bot.username}</span>
              <span className="rounded-full bg-blue-100 px-2 py-0.5 text-[10px] font-medium text-blue-700 dark:bg-blue-900/30 dark:text-blue-400">
                Bot
              </span>
              {bot.deactivatedAt && (
                <span className="rounded-full bg-red-100 px-2 py-0.5 text-[10px] font-medium text-red-700 dark:bg-red-900/30 dark:text-red-400">
                  Deactivated
                </span>
              )}
            </div>
            <p className="text-xs text-gray-500 dark:text-gray-400">
              Created {new Date(bot.createdAt).toLocaleDateString()}
            </p>
          </div>
          <Button
            variant="secondary"
            onClick={() => {
              if (window.confirm(`Deactivate bot "${bot.displayName}"?`)) {
                deleteMutation.mutate(bot.id);
              }
            }}
            disabled={!!bot.deactivatedAt}
          >
            Deactivate
          </Button>
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
              src={emojiImageSrc(emoji)}
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
