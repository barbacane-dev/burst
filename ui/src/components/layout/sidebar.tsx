import { useEffect, useState, type FormEvent } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { Hash, LogOut, MessageSquare, Plus, X, MessageCircle, Search, Sun, Moon, Settings, Shield } from "lucide-react";
import { useQuery, useQueries, useMutation, useQueryClient } from "@tanstack/react-query";
import { useWsEvent } from "../../lib/ws/hooks";
import { useAuth } from "../../lib/auth/context";
import { useTheme } from "../../lib/theme";
import { listChannels, listMembers, createChannel, createDm, browseChannels, joinChannel } from "../../lib/api/channels";
import { listUsers } from "../../lib/api/users";
import { Avatar } from "../ui/avatar";
import { Button } from "../ui/button";
import { Input } from "../ui/input";
import { SearchDialog } from "../search-dialog";
import type { Channel, ChannelMember, PaginatedResponse, User } from "../../lib/api/types";

export function Sidebar() {
  const { user, logout } = useAuth();
  const { resolved, setTheme } = useTheme();
  const navigate = useNavigate();
  const { channelId } = useParams();
  const [showCreate, setShowCreate] = useState(false);
  const [showBrowse, setShowBrowse] = useState(false);
  const [showDm, setShowDm] = useState(false);
  const [showSearch, setShowSearch] = useState(false);

  // Global Cmd+K / Ctrl+K shortcut to open search.
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if ((e.metaKey || e.ctrlKey) && e.key === "k") {
        e.preventDefault();
        setShowSearch((prev) => !prev);
      }
    }
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, []);

  const queryClient = useQueryClient();

  const { data } = useQuery<PaginatedResponse<Channel>>({
    queryKey: ["channels"],
    queryFn: listChannels,
  });

  // Refresh the channel list when a new DM is created and this user is added.
  useWsEvent("channel.joined", () => {
    queryClient.invalidateQueries({ queryKey: ["channels"] });
  });

  const publicChannels = data?.items.filter((ch) => ch.kind === "public" || ch.kind === "private") ?? [];
  const dmChannels = data?.items.filter((ch) => ch.kind === "dm" || ch.kind === "group_dm") ?? [];

  const { data: usersData } = useQuery<PaginatedResponse<User>>({
    queryKey: ["users"],
    queryFn: () => listUsers(),
    staleTime: 60_000,
  });
  const usersById = new Map(usersData?.items.map((u) => [u.id, u.displayName]) ?? []);

  const dmMembersQueries = useQueries({
    queries: dmChannels.map((ch) => ({
      queryKey: ["members", ch.id] as const,
      queryFn: () => listMembers(ch.id),
      staleTime: 60_000,
    })),
  });

  const dmLabels = new Map<string, string>(
    dmChannels.flatMap((ch, i) => {
      const members: ChannelMember[] = dmMembersQueries[i]?.data ?? [];
      const partner = members.find((m) => m.userId !== user?.id);
      if (!partner) return [];
      const name = usersById.get(partner.userId) ?? partner.userId.replace("usr_", "").slice(0, 8);
      return [[ch.id, name]];
    }),
  );

  function dmLabel(ch: Channel): string {
    return ch.name ?? dmLabels.get(ch.id) ?? "Direct Message";
  }

  return (
    <aside className="flex h-full w-64 flex-col border-r border-gray-200 bg-gray-50 dark:border-gray-700 dark:bg-gray-900">
      <div className="flex h-14 items-center justify-between border-b border-gray-200 px-4 dark:border-gray-700">
        <div className="flex items-center">
          <MessageSquare className="mr-2 h-5 w-5 text-indigo-600" />
          <h1 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
            Burst
          </h1>
        </div>
        <button
          onClick={() => setShowSearch(true)}
          className="rounded p-1 text-gray-400 hover:bg-gray-200 hover:text-gray-600 dark:hover:bg-gray-700 dark:hover:text-gray-300"
          title="Search messages (Cmd+K)"
        >
          <Search className="h-4 w-4" />
        </button>
      </div>

      <nav className="flex-1 overflow-y-auto p-3" role="navigation">
        {/* ── Channels ─────────────────────────────────────────── */}
        <div className="mb-2 flex items-center justify-between px-2">
          <h2 className="text-xs font-semibold uppercase tracking-wider text-gray-500 dark:text-gray-400">
            Channels
          </h2>
          <div className="flex items-center gap-0.5">
            <button
              onClick={() => setShowBrowse(true)}
              className="rounded p-0.5 text-gray-400 hover:bg-gray-200 hover:text-gray-600 dark:hover:bg-gray-700 dark:hover:text-gray-300"
              title="Browse channels"
            >
              <Search className="h-4 w-4" />
            </button>
            <button
              onClick={() => setShowCreate(true)}
              className="rounded p-0.5 text-gray-400 hover:bg-gray-200 hover:text-gray-600 dark:hover:bg-gray-700 dark:hover:text-gray-300"
              title="Create channel"
            >
              <Plus className="h-4 w-4" />
            </button>
          </div>
        </div>

        {publicChannels.length === 0 && (
          <p className="px-2 text-sm italic text-gray-400 dark:text-gray-500">
            No channels yet
          </p>
        )}

        {publicChannels.map((ch) => (
          <ChannelNavItem
            key={ch.id}
            channel={ch}
            isActive={channelId === ch.id}
            onClick={() => navigate(`/channels/${ch.id}`)}
            icon={<Hash className="mr-2 h-4 w-4 shrink-0 text-gray-400" />}
            label={ch.name ?? ch.slug ?? "unnamed"}
          />
        ))}

        {/* ── Direct Messages ──────────────────────────────────── */}
        <div className="mb-2 mt-4 flex items-center justify-between px-2">
          <h2 className="text-xs font-semibold uppercase tracking-wider text-gray-500 dark:text-gray-400">
            Direct Messages
          </h2>
          <button
            onClick={() => setShowDm(true)}
            className="rounded p-0.5 text-gray-400 hover:bg-gray-200 hover:text-gray-600 dark:hover:bg-gray-700 dark:hover:text-gray-300"
            title="New direct message"
          >
            <Plus className="h-4 w-4" />
          </button>
        </div>

        {dmChannels.length === 0 && (
          <p className="px-2 text-sm italic text-gray-400 dark:text-gray-500">
            No messages yet
          </p>
        )}

        {dmChannels.map((ch) => (
          <ChannelNavItem
            key={ch.id}
            channel={ch}
            isActive={channelId === ch.id}
            onClick={() => navigate(`/channels/${ch.id}`)}
            icon={<MessageCircle className="mr-2 h-4 w-4 shrink-0 text-gray-400" />}
            label={dmLabel(ch)}
          />
        ))}
      </nav>

      {user && (
        <div className="border-t border-gray-200 p-3 dark:border-gray-700">
          <div className="flex items-center gap-2">
            <Avatar
              name={user.displayName}
              src={user.avatarUrl}
              size="sm"
            />
            <div className="min-w-0 flex-1">
              <p className="truncate text-sm font-medium text-gray-900 dark:text-gray-100">
                {user.displayName}
              </p>
              <p className="truncate text-xs text-gray-500 dark:text-gray-400">
                {user.username}
              </p>
            </div>
            {user.role === "admin" && (
              <button
                onClick={() => navigate("/admin")}
                className="rounded p-1 text-gray-400 hover:bg-gray-200 hover:text-gray-600 dark:hover:bg-gray-700 dark:hover:text-gray-300"
                title="Administration"
              >
                <Shield className="h-4 w-4" />
              </button>
            )}
            <button
              onClick={() => navigate("/settings")}
              className="rounded p-1 text-gray-400 hover:bg-gray-200 hover:text-gray-600 dark:hover:bg-gray-700 dark:hover:text-gray-300"
              title="Settings"
            >
              <Settings className="h-4 w-4" />
            </button>
            <button
              onClick={() => setTheme(resolved === "dark" ? "light" : "dark")}
              className="rounded p-1 text-gray-400 hover:bg-gray-200 hover:text-gray-600 dark:hover:bg-gray-700 dark:hover:text-gray-300"
              title={resolved === "dark" ? "Switch to light mode" : "Switch to dark mode"}
            >
              {resolved === "dark" ? <Sun className="h-4 w-4" /> : <Moon className="h-4 w-4" />}
            </button>
            <button
              onClick={logout}
              className="rounded p-1 text-gray-400 hover:bg-gray-200 hover:text-gray-600 dark:hover:bg-gray-700 dark:hover:text-gray-300"
              title="Log out"
            >
              <LogOut className="h-4 w-4" />
            </button>
          </div>
        </div>
      )}

      {showCreate && (
        <CreateChannelDialog onClose={() => setShowCreate(false)} />
      )}

      {showBrowse && (
        <BrowseChannelsDialog onClose={() => setShowBrowse(false)} />
      )}

      {showDm && (
        <NewDmDialog onClose={() => setShowDm(false)} currentUserId={user?.id ?? ""} />
      )}

      {showSearch && (
        <SearchDialog onClose={() => setShowSearch(false)} />
      )}
    </aside>
  );
}

function ChannelNavItem({
  channel,
  isActive,
  onClick,
  icon,
  label,
}: {
  channel: Channel;
  isActive: boolean;
  onClick: () => void;
  icon: React.ReactNode;
  label: string;
}) {
  const hasUnread = channel.unreadCount > 0;

  return (
    <button
      onClick={onClick}
      className={`flex w-full items-center rounded-md px-2 py-1.5 text-sm transition-colors ${
        isActive
          ? "bg-indigo-100 text-indigo-900 dark:bg-indigo-900/30 dark:text-indigo-200"
          : "text-gray-700 hover:bg-gray-200 dark:text-gray-300 dark:hover:bg-gray-800"
      }`}
    >
      {icon}
      <span className={`truncate flex-1 text-left ${hasUnread && !isActive ? "font-semibold" : ""}`}>
        {label}
      </span>
      {hasUnread && !isActive && (
        <span className="ml-1 flex h-4 min-w-4 items-center justify-center rounded-full bg-indigo-600 px-1 text-[10px] font-bold text-white">
          {channel.unreadCount > 99 ? "99+" : channel.unreadCount}
        </span>
      )}
    </button>
  );
}

function CreateChannelDialog({ onClose }: { onClose: () => void }) {
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const [name, setName] = useState("");
  const [error, setError] = useState("");

  const mutation = useMutation({
    mutationFn: (channelName: string) => createChannel({ name: channelName }),
    onSuccess: (channel) => {
      queryClient.invalidateQueries({ queryKey: ["channels"] });
      navigate(`/channels/${channel.id}`);
      onClose();
    },
    onError: (err: Error) => {
      setError(err.message);
    },
  });

  function handleSubmit(e: FormEvent) {
    e.preventDefault();
    const trimmed = name.trim();
    if (!trimmed) return;
    setError("");
    mutation.mutate(trimmed);
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="w-full max-w-sm rounded-lg bg-white p-6 shadow-xl dark:bg-gray-800">
        <div className="mb-4 flex items-center justify-between">
          <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
            Create Channel
          </h3>
          <button
            onClick={onClose}
            className="rounded p-1 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
          >
            <X className="h-5 w-5" />
          </button>
        </div>

        <form onSubmit={handleSubmit} className="space-y-4">
          {error && (
            <div className="rounded-md bg-red-50 p-3 text-sm text-red-700 dark:bg-red-900/30 dark:text-red-400">
              {error}
            </div>
          )}

          <Input
            id="channel-name"
            label="Channel name"
            required
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="e.g. engineering"
            autoFocus
          />

          <div className="flex justify-end gap-2">
            <Button variant="secondary" type="button" onClick={onClose}>
              Cancel
            </Button>
            <Button type="submit" disabled={mutation.isPending}>
              Create
            </Button>
          </div>
        </form>
      </div>
    </div>
  );
}

function BrowseChannelsDialog({ onClose }: { onClose: () => void }) {
  const queryClient = useQueryClient();
  const navigate = useNavigate();

  const { data, isLoading } = useQuery<PaginatedResponse<Channel>>({
    queryKey: ["channels-browse"],
    queryFn: browseChannels,
  });

  const joinedIds = new Set(
    (queryClient.getQueryData<PaginatedResponse<Channel>>(["channels"])?.items ?? []).map(
      (ch) => ch.id,
    ),
  );

  const mutation = useMutation({
    mutationFn: (channelId: string) => joinChannel(channelId),
    onSuccess: (_void, channelId) => {
      queryClient.invalidateQueries({ queryKey: ["channels"] });
      navigate(`/channels/${channelId}`);
      onClose();
    },
  });

  const channels = data?.items ?? [];

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="w-full max-w-sm rounded-lg bg-white p-6 shadow-xl dark:bg-gray-800">
        <div className="mb-4 flex items-center justify-between">
          <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
            Browse Channels
          </h3>
          <button
            onClick={onClose}
            className="rounded p-1 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
          >
            <X className="h-5 w-5" />
          </button>
        </div>

        {isLoading ? (
          <p className="text-sm text-gray-400">Loading…</p>
        ) : channels.length === 0 ? (
          <p className="text-sm text-gray-400">No public channels found.</p>
        ) : (
          <ul className="max-h-64 overflow-y-auto space-y-1">
            {channels.map((ch) => {
              const joined = joinedIds.has(ch.id);
              return (
                <li key={ch.id} className="flex items-center justify-between gap-2 rounded-md px-3 py-2 hover:bg-gray-50 dark:hover:bg-gray-700">
                  <div className="flex min-w-0 items-center gap-2">
                    <Hash className="h-4 w-4 shrink-0 text-gray-400" />
                    <div className="min-w-0">
                      <p className="truncate text-sm font-medium text-gray-900 dark:text-gray-100">
                        {ch.name ?? ch.slug ?? "unnamed"}
                      </p>
                      {ch.topic && (
                        <p className="truncate text-xs text-gray-500 dark:text-gray-400">{ch.topic}</p>
                      )}
                    </div>
                  </div>
                  {joined ? (
                    <span className="shrink-0 text-xs text-gray-400">Joined</span>
                  ) : (
                    <button
                      onClick={() => mutation.mutate(ch.id)}
                      disabled={mutation.isPending}
                      className="shrink-0 rounded-md bg-indigo-600 px-2 py-1 text-xs font-medium text-white hover:bg-indigo-500 disabled:opacity-50"
                    >
                      Join
                    </button>
                  )}
                </li>
              );
            })}
          </ul>
        )}
      </div>
    </div>
  );
}

function NewDmDialog({
  onClose,
  currentUserId,
}: {
  onClose: () => void;
  currentUserId: string;
}) {
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const [error, setError] = useState("");

  const { data: usersData, isLoading } = useQuery<PaginatedResponse<User>>({
    queryKey: ["users"],
    queryFn: () => listUsers(),
  });

  const users = (usersData?.items ?? []).filter((u) => u.id !== currentUserId);

  const mutation = useMutation({
    mutationFn: (userId: string) => createDm(userId),
    onSuccess: (channel) => {
      queryClient.invalidateQueries({ queryKey: ["channels"] });
      navigate(`/channels/${channel.id}`);
      onClose();
    },
    onError: (err: Error) => {
      setError(err.message);
    },
  });

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="w-full max-w-sm rounded-lg bg-white p-6 shadow-xl dark:bg-gray-800">
        <div className="mb-4 flex items-center justify-between">
          <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
            New Direct Message
          </h3>
          <button
            onClick={onClose}
            className="rounded p-1 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
          >
            <X className="h-5 w-5" />
          </button>
        </div>

        {error && (
          <div className="mb-3 rounded-md bg-red-50 p-3 text-sm text-red-700 dark:bg-red-900/30 dark:text-red-400">
            {error}
          </div>
        )}

        {isLoading ? (
          <p className="text-sm text-gray-400">Loading users…</p>
        ) : users.length === 0 ? (
          <p className="text-sm text-gray-400">No other users found.</p>
        ) : (
          <ul className="max-h-64 overflow-y-auto space-y-1">
            {users.map((u) => (
              <li key={u.id}>
                <button
                  onClick={() => { setError(""); mutation.mutate(u.id); }}
                  disabled={mutation.isPending}
                  className="flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm text-left hover:bg-gray-100 dark:hover:bg-gray-700 disabled:opacity-50"
                >
                  <Avatar name={u.displayName} src={u.avatarUrl} size="sm" />
                  <div>
                    <p className="font-medium text-gray-900 dark:text-gray-100">{u.displayName}</p>
                    <p className="text-xs text-gray-500">@{u.username}</p>
                  </div>
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}
