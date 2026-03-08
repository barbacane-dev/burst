import { useState, type FormEvent } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { Hash, LogOut, MessageSquare, Plus, X } from "lucide-react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { useAuth } from "../../lib/auth/context";
import { listChannels, createChannel } from "../../lib/api/channels";
import { Avatar } from "../ui/avatar";
import { Button } from "../ui/button";
import { Input } from "../ui/input";
import type { Channel, PaginatedResponse } from "../../lib/api/types";

export function Sidebar() {
  const { user, logout } = useAuth();
  const navigate = useNavigate();
  const { channelId } = useParams();
  const [showCreate, setShowCreate] = useState(false);

  const { data } = useQuery<PaginatedResponse<Channel>>({
    queryKey: ["channels"],
    queryFn: listChannels,
  });
  const channels = data?.items ?? [];

  return (
    <aside className="flex h-full w-64 flex-col border-r border-gray-200 bg-gray-50 dark:border-gray-700 dark:bg-gray-900">
      <div className="flex h-14 items-center border-b border-gray-200 px-4 dark:border-gray-700">
        <MessageSquare className="mr-2 h-5 w-5 text-indigo-600" />
        <h1 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
          Burst
        </h1>
      </div>

      <nav className="flex-1 overflow-y-auto p-3" role="navigation">
        <div className="mb-2 flex items-center justify-between px-2">
          <h2 className="text-xs font-semibold uppercase tracking-wider text-gray-500 dark:text-gray-400">
            Channels
          </h2>
          <button
            onClick={() => setShowCreate(true)}
            className="rounded p-0.5 text-gray-400 hover:bg-gray-200 hover:text-gray-600 dark:hover:bg-gray-700 dark:hover:text-gray-300"
            title="Create channel"
          >
            <Plus className="h-4 w-4" />
          </button>
        </div>

        {channels.length === 0 && (
          <p className="px-2 text-sm italic text-gray-400 dark:text-gray-500">
            No channels yet
          </p>
        )}

        {channels.map((ch) => (
          <button
            key={ch.id}
            onClick={() => navigate(`/channels/${ch.id}`)}
            className={`flex w-full items-center rounded-md px-2 py-1.5 text-sm transition-colors ${
              channelId === ch.id
                ? "bg-indigo-100 text-indigo-900 dark:bg-indigo-900/30 dark:text-indigo-200"
                : "text-gray-700 hover:bg-gray-200 dark:text-gray-300 dark:hover:bg-gray-800"
            }`}
          >
            <Hash className="mr-2 h-4 w-4 shrink-0 text-gray-400" />
            <span className="truncate">{ch.name ?? ch.slug ?? "unnamed"}</span>
          </button>
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
    </aside>
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
