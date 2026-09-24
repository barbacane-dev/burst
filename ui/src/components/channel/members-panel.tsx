import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Users, X, UserPlus } from "lucide-react";
import {
  addMember,
  archiveChannel,
  leaveChannel,
  listMembers,
  removeMember,
  setMemberRole,
  unarchiveChannel,
} from "../../lib/api/channels";
import { Avatar } from "../ui/avatar";
import { Spinner } from "../ui/spinner";
import { can, canRemove, canSetRole, type Actor, type ChannelRole } from "../../lib/permissions";
import type { Channel, ChannelMember, User } from "../../lib/api/types";
import { UserStatusLine } from "../ui/user-status";

const ROLE_LABEL: Record<ChannelRole, string | null> = {
  owner: "Owner",
  moderator: "Moderator",
  member: null,
};

function errorText(e: unknown): string {
  return e instanceof Error ? e.message : "Something went wrong";
}

export function MembersPanel({
  channel,
  currentUser,
  users,
  onClose,
}: {
  channel: Channel;
  currentUser: User;
  users: User[];
  onClose: () => void;
}) {
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const [adding, setAdding] = useState(false);
  const [filter, setFilter] = useState("");

  const { data: members, isLoading } = useQuery<ChannelMember[]>({
    queryKey: ["members", channel.id],
    queryFn: () => listMembers(channel.id),
  });

  const usersById = new Map(users.map((u) => [u.id, u]));
  const mine = members?.find((m) => m.userId === currentUser.id);
  const actor: Actor = { instance: currentUser.role, channel: mine?.role };
  const isNamedChannel = channel.kind === "public" || channel.kind === "private";

  const refresh = () => {
    queryClient.invalidateQueries({ queryKey: ["members", channel.id] });
    queryClient.invalidateQueries({ queryKey: ["channels"] });
    queryClient.invalidateQueries({ queryKey: ["channel", channel.id] });
  };

  const add = useMutation({
    mutationFn: (userId: string) => addMember(channel.id, userId),
    onSuccess: refresh,
  });
  const remove = useMutation({
    mutationFn: (userId: string) => removeMember(channel.id, userId),
    onSuccess: refresh,
  });
  const setRole = useMutation({
    mutationFn: ({ userId, role }: { userId: string; role: "moderator" | "member" }) =>
      setMemberRole(channel.id, userId, role),
    onSuccess: refresh,
  });
  const leave = useMutation({
    mutationFn: () => leaveChannel(channel.id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["channels"] });
      navigate("/");
    },
  });
  const archive = useMutation({
    mutationFn: () => (channel.isArchived ? unarchiveChannel(channel.id) : archiveChannel(channel.id)),
    onSuccess: refresh,
  });

  const memberIds = new Set(members?.map((m) => m.userId));
  const candidates = users
    .filter((u) => !memberIds.has(u.id) && !u.isBot)
    .filter((u) => {
      const q = filter.toLowerCase();
      return u.displayName.toLowerCase().includes(q) || u.username.toLowerCase().includes(q);
    })
    .slice(0, 20);

  const failure = add.error ?? remove.error ?? setRole.error ?? leave.error ?? archive.error;

  return (
    <aside
      aria-label="Channel members"
      className="flex w-80 shrink-0 flex-col border-l border-gray-200 bg-white dark:border-gray-700 dark:bg-gray-900"
    >
      <div className="flex h-14 items-center justify-between border-b border-gray-200 px-4 dark:border-gray-700">
        <div className="flex items-center gap-2">
          <Users className="h-4 w-4 text-gray-400" />
          <span className="text-sm font-semibold text-gray-900 dark:text-gray-100">
            Members{members ? ` (${members.length})` : ""}
          </span>
        </div>
        <button
          onClick={onClose}
          aria-label="Close members"
          className="rounded p-1 text-gray-400 hover:bg-gray-100 hover:text-gray-600 dark:hover:bg-gray-700"
        >
          <X className="h-4 w-4" />
        </button>
      </div>

      {isNamedChannel && can(actor, "addMember") && (
        <div className="border-b border-gray-200 p-3 dark:border-gray-700">
          {adding ? (
            <div className="space-y-2">
              <input
                autoFocus
                value={filter}
                onChange={(e) => setFilter(e.target.value)}
                placeholder="Find someone to add"
                aria-label="Find someone to add"
                className="w-full rounded-md border border-gray-300 px-2 py-1 text-sm dark:border-gray-600 dark:bg-gray-800"
              />
              <ul className="max-h-48 overflow-y-auto" aria-label="People you can add">
                {candidates.map((u) => (
                  <li key={u.id}>
                    <button
                      onClick={() => add.mutate(u.id)}
                      aria-label={`Add ${u.displayName}`}
                      className="flex w-full items-center gap-2 rounded px-2 py-1 text-left text-sm hover:bg-gray-100 dark:hover:bg-gray-800"
                    >
                      <Avatar name={u.displayName} src={u.avatarUrl} size="sm" />
                      <span className="truncate">{u.displayName}</span>
                      <span className="truncate text-xs text-gray-500">@{u.username}</span>
                    </button>
                  </li>
                ))}
                {candidates.length === 0 && (
                  <li className="px-2 py-1 text-xs text-gray-400">Nobody else to add</li>
                )}
              </ul>
              <button onClick={() => setAdding(false)} className="text-xs text-gray-500 hover:underline">
                Done
              </button>
            </div>
          ) : (
            <button
              onClick={() => setAdding(true)}
              className="flex items-center gap-2 text-sm font-medium text-indigo-600 hover:text-indigo-500 dark:text-indigo-400"
            >
              <UserPlus className="h-4 w-4" />
              Add people
            </button>
          )}
        </div>
      )}

      {failure && (
        <p role="alert" className="border-b border-red-200 bg-red-50 px-3 py-2 text-xs text-red-700 dark:border-red-900 dark:bg-red-950 dark:text-red-300">
          {errorText(failure)}
        </p>
      )}

      <ul className="flex-1 space-y-1 overflow-y-auto p-3" aria-label="Members">
        {isLoading ? (
          <li className="flex justify-center py-4">
            <Spinner className="h-5 w-5 text-indigo-600" />
          </li>
        ) : (
          members?.map((m) => {
            const u = usersById.get(m.userId);
            const name = u?.displayName ?? m.userId.replace("usr_", "").slice(0, 8);
            const isMe = m.userId === currentUser.id;
            const label = ROLE_LABEL[m.role];
            const nextRole = m.role === "moderator" ? "member" : "moderator";
            return (
              <li key={m.userId} className="group flex items-center gap-2 rounded px-2 py-1">
                <Avatar name={name} src={u?.avatarUrl} size="sm" />
                <span className="min-w-0 flex-1 text-sm text-gray-900 dark:text-gray-100">
                  <span className="block truncate">
                    {name}
                    {isMe && <span className="text-gray-400"> (you)</span>}
                  </span>
                  <UserStatusLine userId={m.userId} />
                </span>
                {label && (
                  <span className="rounded bg-gray-100 px-1.5 py-0.5 text-xs text-gray-600 dark:bg-gray-800 dark:text-gray-300">
                    {label}
                  </span>
                )}
                {!isMe && isNamedChannel && canSetRole(actor, m.role, nextRole) && (
                  <button
                    onClick={() => setRole.mutate({ userId: m.userId, role: nextRole })}
                    aria-label={nextRole === "moderator" ? `Make ${name} a moderator` : `Make ${name} a member`}
                    className="text-xs text-indigo-600 hover:underline dark:text-indigo-400"
                  >
                    {nextRole === "moderator" ? "Make moderator" : "Make member"}
                  </button>
                )}
                {!isMe && isNamedChannel && canRemove(actor, m.role) && (
                  <button
                    onClick={() => remove.mutate(m.userId)}
                    aria-label={`Remove ${name}`}
                    className="text-xs text-red-600 hover:underline dark:text-red-400"
                  >
                    Remove
                  </button>
                )}
              </li>
            );
          })
        )}
      </ul>

      {isNamedChannel && (
        <div className="space-y-2 border-t border-gray-200 p-3 dark:border-gray-700">
          {can(actor, "archiveChannel") && (
            <button
              onClick={() => archive.mutate()}
              className="block text-sm text-gray-600 hover:underline dark:text-gray-300"
            >
              {channel.isArchived ? "Unarchive channel" : "Archive channel"}
            </button>
          )}
          {mine && (
            <button onClick={() => leave.mutate()} className="block text-sm text-red-600 hover:underline dark:text-red-400">
              Leave channel
            </button>
          )}
        </div>
      )}
    </aside>
  );
}
