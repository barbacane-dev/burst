import {
  useState,
  useRef,
  useEffect,
  useCallback,
  type FormEvent,
  type KeyboardEvent,
} from "react";
import { useParams } from "react-router-dom";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { Hash, Send, MessageSquare, X, Smile, MessageCircle, Paperclip } from "lucide-react";
import {
  getChannel,
  listMembers,
  listMessages,
  listThreadReplies,
  sendMessage,
  addReaction,
  removeReaction,
  markChannelRead,
} from "../lib/api/channels";
import { listUsers } from "../lib/api/users";
import { useAuth } from "../lib/auth/context";
import { Avatar } from "../components/ui/avatar";
import { Spinner } from "../components/ui/spinner";
import { AttachmentPreview } from "../components/attachment-preview";
import type { Channel, ChannelMember, Message, PaginatedResponse, ReactionCount, User } from "../lib/api/types";
import { useWsEvent, useTypingIndicator } from "../lib/ws/hooks";

const EMOJI_PICKER = ["👍", "👎", "❤️", "😂", "😮", "😢", "🎉", "🚀", "👀", "🔥"];

export function ChannelPage() {
  const { channelId } = useParams<{ channelId: string }>();
  const { user } = useAuth();
  const queryClient = useQueryClient();
  const [threadMessageId, setThreadMessageId] = useState<string | null>(null);

  const cachedChannels = queryClient.getQueryData<PaginatedResponse<Channel>>(["channels"]);
  const cachedChannel = cachedChannels?.items.find((ch) => ch.id === channelId);

  const { data: fetchedChannel } = useQuery<Channel>({
    queryKey: ["channel", channelId],
    queryFn: () => getChannel(channelId!),
    enabled: !!channelId && !cachedChannel,
    staleTime: 30_000,
  });

  const channel = cachedChannel ?? fetchedChannel;
  const isDm = channel?.kind === "dm" || channel?.kind === "group_dm";

  // ── User display-name lookup ───────────────────────────────────────────────

  const { data: usersData } = useQuery<PaginatedResponse<User>>({
    queryKey: ["users"],
    queryFn: () => listUsers(),
    staleTime: 60_000,
  });
  const usersById = new Map(usersData?.items.map((u) => [u.id, u.displayName]) ?? []);

  // ── DM title: resolve the other participant's display name ─────────────────

  const { data: members } = useQuery<ChannelMember[]>({
    queryKey: ["members", channelId],
    queryFn: () => listMembers(channelId!),
    enabled: !!channelId && isDm,
    staleTime: 60_000,
  });

  function channelTitle(): string {
    if (!channel) return "Loading...";
    if (isDm) {
      const partner = members?.find((m) => m.userId !== user?.id);
      if (partner) return usersById.get(partner.userId) ?? partner.userId.replace("usr_", "").slice(0, 8);
      return "Direct Message";
    }
    return channel.name ?? "#unnamed";
  }

  // ── Messages ───────────────────────────────────────────────────────────────

  const { data, isLoading } = useQuery<PaginatedResponse<Message>>({
    queryKey: ["messages", channelId],
    queryFn: () => listMessages(channelId!),
    enabled: !!channelId,
    staleTime: Infinity,
  });

  const messages = data?.items ?? [];
  const sorted = [...messages].reverse();

  const bottomRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "instant" });
  }, [sorted.length]);

  // Mark channel as read when visiting; refresh sidebar unread counts
  useEffect(() => {
    if (!channelId) return;
    markChannelRead(channelId).then(() => {
      queryClient.invalidateQueries({ queryKey: ["channels"] });
    });
  }, [channelId, queryClient]);

  // ── Live message delivery ─────────────────────────────────────────────────

  useWsEvent<{ type: string; channelId: string; message: Message }>(
    "message.created",
    (ev) => {
      if (ev.channelId !== channelId) return;
      queryClient.setQueryData<PaginatedResponse<Message>>(
        ["messages", channelId],
        (old) => {
          const hydrated: Message = { ...ev.message, reactions: ev.message.reactions ?? [], attachments: ev.message.attachments ?? [], replyCount: ev.message.replyCount ?? 0 };
          if (!old) return { items: [hydrated], cursor: undefined };
          if (ev.message.threadId) {
            // Increment reply count on the parent in the main view
            return {
              ...old,
              items: old.items.map((m) =>
                m.id === ev.message.threadId ? { ...m, replyCount: m.replyCount + 1 } : m,
              ),
            };
          }
          const exists = old.items.some((m) => m.id === ev.message.id);
          if (exists) {
            return { ...old, items: old.items.map((m) => m.id === ev.message.id ? hydrated : m) };
          }
          return { ...old, items: [hydrated, ...old.items] };
        },
      );
      // Also append to open thread cache
      if (ev.message.threadId && ev.message.threadId === threadMessageId) {
        queryClient.setQueryData<PaginatedResponse<Message>>(
          ["thread", threadMessageId],
          (old) => {
            const hydratedReply: Message = { ...ev.message, reactions: ev.message.reactions ?? [], attachments: ev.message.attachments ?? [], replyCount: ev.message.replyCount ?? 0 };
            if (!old) return { items: [hydratedReply], cursor: undefined };
            if (old.items.some((m) => m.id === ev.message.id)) return old;
            return { ...old, items: [...old.items, hydratedReply] };
          },
        );
      }
    },
  );

  useWsEvent<{ type: string; channelId: string; message: Message }>(
    "message.updated",
    (ev) => {
      if (ev.channelId !== channelId) return;
      queryClient.setQueryData<PaginatedResponse<Message>>(
        ["messages", channelId],
        (old) =>
          old
            ? { ...old, items: old.items.map((m) => m.id === ev.message.id ? ev.message : m) }
            : old,
      );
    },
  );

  useWsEvent<{ type: string; channelId: string; messageId: string }>(
    "message.deleted",
    (ev) => {
      if (ev.channelId !== channelId) return;
      queryClient.setQueryData<PaginatedResponse<Message>>(
        ["messages", channelId],
        (old) =>
          old
            ? {
                ...old,
                items: old.items.map((m) =>
                  m.id === ev.messageId
                    ? { ...m, deletedAt: new Date().toISOString(), content: "" }
                    : m,
                ),
              }
            : old,
      );
    },
  );

  // ── Reaction events ───────────────────────────────────────────────────────

  useWsEvent<{ type: string; channelId: string; messageId: string; emoji: string; userId: string }>(
    "reaction.added",
    (ev) => {
      if (ev.channelId !== channelId) return;
      const updater = (old: PaginatedResponse<Message> | undefined) =>
        old
          ? {
              ...old,
              items: old.items.map((m) =>
                m.id === ev.messageId ? addReactionLocally(m, ev.emoji, ev.userId) : m,
              ),
            }
          : old;
      queryClient.setQueryData<PaginatedResponse<Message>>(["messages", channelId], updater);
      if (threadMessageId) {
        queryClient.setQueryData<PaginatedResponse<Message>>(["thread", threadMessageId], updater);
      }
    },
  );

  useWsEvent<{ type: string; channelId: string; messageId: string; emoji: string; userId: string }>(
    "reaction.removed",
    (ev) => {
      if (ev.channelId !== channelId) return;
      const updater = (old: PaginatedResponse<Message> | undefined) =>
        old
          ? {
              ...old,
              items: old.items.map((m) =>
                m.id === ev.messageId ? removeReactionLocally(m, ev.emoji, ev.userId) : m,
              ),
            }
          : old;
      queryClient.setQueryData<PaginatedResponse<Message>>(["messages", channelId], updater);
      if (threadMessageId) {
        queryClient.setQueryData<PaginatedResponse<Message>>(["thread", threadMessageId], updater);
      }
    },
  );

  // ── Typing indicator ──────────────────────────────────────────────────────

  const { typingUsers, sendTypingStart, sendTypingStop } = useTypingIndicator(channelId ?? "", user?.id);

  const openThread = useCallback((messageId: string) => {
    setThreadMessageId(messageId);
  }, []);

  const closeThread = useCallback(() => {
    setThreadMessageId(null);
  }, []);

  return (
    <div className="flex flex-1 overflow-hidden">
      <div className="flex flex-1 flex-col min-w-0">
        <header className="flex h-14 items-center border-b border-gray-200 px-4 dark:border-gray-700">
          {isDm
            ? <MessageCircle className="mr-2 h-5 w-5 text-gray-400" />
            : <Hash className="mr-2 h-5 w-5 text-gray-400" />
          }
          <h2 className="text-base font-semibold text-gray-900 dark:text-gray-100">
            {channelTitle()}
          </h2>
          {channel?.topic && (
            <span className="ml-3 text-sm text-gray-500 dark:text-gray-400">
              {channel.topic}
            </span>
          )}
        </header>

        <div className="flex-1 overflow-y-auto px-4 py-2">
          {isLoading ? (
            <div className="flex h-full items-center justify-center">
              <Spinner className="h-6 w-6 text-indigo-600" />
            </div>
          ) : sorted.length === 0 ? (
            <div className="flex h-full items-center justify-center">
              <div className="text-center text-gray-400 dark:text-gray-500">
                <p className="text-lg font-medium">No messages yet</p>
                <p className="text-sm">Be the first to say something!</p>
              </div>
            </div>
          ) : (
            <div className="space-y-1">
              {sorted.map((msg) => (
                <MessageBubble
                  key={msg.id}
                  message={msg}
                  currentUserId={user?.id ?? ""}
                  channelId={channelId!}
                  usersById={usersById}
                  onOpenThread={() => openThread(msg.id)}
                  isThreadOpen={threadMessageId === msg.id}
                />
              ))}
              <div ref={bottomRef} />
            </div>
          )}
        </div>

        {typingUsers.size > 0 && (
          <div className="px-4 py-1 text-xs text-gray-400 dark:text-gray-500">
            {[...typingUsers]
              .map((id) => usersById.get(id) ?? id.replace("usr_", "").slice(0, 8))
              .join(", ")}{" "}
            {typingUsers.size === 1 ? "is" : "are"} typing...
          </div>
        )}

        {channelId && (
          <MessageComposer
            channelId={channelId}
            onTypingStart={sendTypingStart}
            onTypingStop={sendTypingStop}
          />
        )}
      </div>

      {threadMessageId && channelId && (
        <ThreadPanel
          channelId={channelId}
          threadMessageId={threadMessageId}
          currentUserId={user?.id ?? ""}
          usersById={usersById}
          onClose={closeThread}
        />
      )}
    </div>
  );
}

// ── Reaction helpers ──────────────────────────────────────────────────────────

function addReactionLocally(msg: Message, emoji: string, userId: string): Message {
  const existing = msg.reactions.find((r) => r.emoji === emoji);
  if (existing) {
    if (existing.userIds.includes(userId)) return msg;
    return {
      ...msg,
      reactions: msg.reactions.map((r) =>
        r.emoji === emoji ? { ...r, count: r.count + 1, userIds: [...r.userIds, userId] } : r,
      ),
    };
  }
  return { ...msg, reactions: [...msg.reactions, { emoji, count: 1, userIds: [userId] }] };
}

function removeReactionLocally(msg: Message, emoji: string, userId: string): Message {
  return {
    ...msg,
    reactions: msg.reactions
      .map((r) =>
        r.emoji === emoji
          ? { ...r, count: r.count - 1, userIds: r.userIds.filter((id) => id !== userId) }
          : r,
      )
      .filter((r) => r.count > 0),
  };
}

// ── MessageBubble ─────────────────────────────────────────────────────────────

function MessageBubble({
  message,
  currentUserId,
  channelId,
  threadId,
  usersById,
  onOpenThread,
  showThreadButton = true,
  isThreadOpen = false,
}: {
  message: Message;
  currentUserId: string;
  channelId: string;
  threadId?: string;
  usersById: Map<string, string>;
  onOpenThread?: () => void;
  showThreadButton?: boolean;
  isThreadOpen?: boolean;
}) {
  const isDeleted = !!message.deletedAt;
  const [showPicker, setShowPicker] = useState(false);
  const queryClient = useQueryClient();

  const reactionMutation = useMutation({
    mutationFn: ({ emoji, hasReacted }: { emoji: string; hasReacted: boolean }) =>
      hasReacted
        ? removeReaction(channelId, message.id, emoji)
        : addReaction(channelId, message.id, emoji),
  });

  function toggleReaction(emoji: string) {
    const existing = message.reactions.find((r) => r.emoji === emoji);
    const hasReacted = existing?.userIds.includes(currentUserId) ?? false;
    // Optimistic update — target the thread cache when inside a thread view
    const cacheKey: unknown[] = threadId ? ["thread", threadId] : ["messages", channelId];
    queryClient.setQueryData<PaginatedResponse<Message>>(
      cacheKey,
      (old) =>
        old
          ? {
              ...old,
              items: old.items.map((m) =>
                m.id === message.id
                  ? hasReacted
                    ? removeReactionLocally(m, emoji, currentUserId)
                    : addReactionLocally(m, emoji, currentUserId)
                  : m,
              ),
            }
          : old,
    );
    reactionMutation.mutate({ emoji, hasReacted });
    setShowPicker(false);
  }

  const displayName = usersById.get(message.userId) ?? message.userId.replace("usr_", "").slice(0, 8);

  return (
    <div
      className={`group relative flex items-start gap-3 rounded-md px-2 py-1.5 hover:bg-gray-50 dark:hover:bg-gray-900/50 ${
        isThreadOpen ? "bg-indigo-50/50 dark:bg-indigo-900/10" : ""
      }`}
    >
      <Avatar name={displayName} size="sm" />
      <div className="min-w-0 flex-1">
        <div className="flex items-baseline gap-2">
          <span className="text-sm font-semibold text-gray-900 dark:text-gray-100">
            {displayName}
          </span>
          <time className="text-xs text-gray-400 dark:text-gray-500">
            {formatTime(message.createdAt)}
          </time>
          {message.editedAt && (
            <span className="text-xs text-gray-400 dark:text-gray-500">(edited)</span>
          )}
        </div>

        <p
          className={`text-sm ${
            isDeleted
              ? "italic text-gray-400 dark:text-gray-500"
              : "text-gray-800 dark:text-gray-200"
          }`}
        >
          {isDeleted ? "This message was deleted" : message.content}
        </p>

        {/* Attachments */}
        {!isDeleted && message.attachments?.length > 0 && (
          <div className="mt-1 flex flex-col gap-1">
            {message.attachments.map((att) => (
              <AttachmentPreview key={att.id} attachment={att} />
            ))}
          </div>
        )}

        {/* Reactions */}
        {message.reactions.length > 0 && (
          <div className="mt-1 flex flex-wrap gap-1">
            {message.reactions.map((r) => (
              <ReactionPill
                key={r.emoji}
                reaction={r}
                currentUserId={currentUserId}
                onClick={() => toggleReaction(r.emoji)}
              />
            ))}
          </div>
        )}

        {/* Thread reply count — only in main channel view */}
        {showThreadButton && !isDeleted && message.replyCount > 0 && (
          <button
            onClick={onOpenThread}
            className="mt-1 text-xs text-indigo-600 hover:underline dark:text-indigo-400"
          >
            {message.replyCount} {message.replyCount === 1 ? "reply" : "replies"}
          </button>
        )}
      </div>

      {/* Hover actions */}
      {!isDeleted && (
        <div className="absolute right-2 top-1 hidden items-center gap-1 rounded-md border border-gray-200 bg-white p-0.5 shadow-sm group-hover:flex dark:border-gray-700 dark:bg-gray-800">
          <div className="relative">
            <button
              onClick={() => setShowPicker((p) => !p)}
              className="rounded p-1 text-gray-400 hover:bg-gray-100 hover:text-gray-600 dark:hover:bg-gray-700"
              title="Add reaction"
            >
              <Smile className="h-4 w-4" />
            </button>
            {showPicker && (
              <EmojiPickerDropdown
                onSelect={toggleReaction}
                onClose={() => setShowPicker(false)}
              />
            )}
          </div>
          {showThreadButton && (
            <button
              onClick={onOpenThread}
              className="rounded p-1 text-gray-400 hover:bg-gray-100 hover:text-gray-600 dark:hover:bg-gray-700"
              title="Reply in thread"
            >
              <MessageSquare className="h-4 w-4" />
            </button>
          )}
        </div>
      )}
    </div>
  );
}

function ReactionPill({
  reaction,
  currentUserId,
  onClick,
}: {
  reaction: ReactionCount;
  currentUserId: string;
  onClick: () => void;
}) {
  const hasReacted = reaction.userIds.includes(currentUserId);
  return (
    <button
      onClick={onClick}
      className={`flex items-center gap-1 rounded-full border px-2 py-0.5 text-xs transition-colors ${
        hasReacted
          ? "border-indigo-400 bg-indigo-50 text-indigo-700 dark:border-indigo-600 dark:bg-indigo-900/30 dark:text-indigo-300"
          : "border-gray-200 bg-gray-50 text-gray-700 hover:border-gray-300 dark:border-gray-700 dark:bg-gray-800 dark:text-gray-300"
      }`}
    >
      <span>{reaction.emoji}</span>
      <span>{reaction.count}</span>
    </button>
  );
}

function EmojiPickerDropdown({
  onSelect,
  onClose,
}: {
  onSelect: (emoji: string) => void;
  onClose: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    function handleClick(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        onClose();
      }
    }
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, [onClose]);

  return (
    <div
      ref={ref}
      className="absolute right-0 top-8 z-10 flex flex-wrap gap-1 rounded-lg border border-gray-200 bg-white p-2 shadow-lg dark:border-gray-700 dark:bg-gray-800"
      style={{ width: "160px" }}
    >
      {EMOJI_PICKER.map((emoji) => (
        <button
          key={emoji}
          onClick={() => onSelect(emoji)}
          className="rounded p-1 text-lg hover:bg-gray-100 dark:hover:bg-gray-700"
        >
          {emoji}
        </button>
      ))}
    </div>
  );
}

// ── Thread Panel ──────────────────────────────────────────────────────────────

function ThreadPanel({
  channelId,
  threadMessageId,
  currentUserId,
  usersById,
  onClose,
}: {
  channelId: string;
  threadMessageId: string;
  currentUserId: string;
  usersById: Map<string, string>;
  onClose: () => void;
}) {
  const queryClient = useQueryClient();
  const bottomRef = useRef<HTMLDivElement>(null);

  const channelMessages = queryClient.getQueryData<PaginatedResponse<Message>>(
    ["messages", channelId],
  );
  const rootMessage = channelMessages?.items.find((m) => m.id === threadMessageId);

  const { data, isLoading } = useQuery<PaginatedResponse<Message>>({
    queryKey: ["thread", threadMessageId],
    queryFn: () => listThreadReplies(channelId, threadMessageId),
    enabled: !!threadMessageId,
    staleTime: Infinity,
  });

  const replies = data?.items ?? [];

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "instant" });
  }, [replies.length]);

  return (
    <div className="flex w-80 shrink-0 flex-col border-l border-gray-200 bg-white dark:border-gray-700 dark:bg-gray-900">
      <div className="flex h-14 items-center justify-between border-b border-gray-200 px-4 dark:border-gray-700">
        <span className="text-sm font-semibold text-gray-900 dark:text-gray-100">Thread</span>
        <button
          onClick={onClose}
          className="rounded p-1 text-gray-400 hover:bg-gray-100 hover:text-gray-600 dark:hover:bg-gray-700"
        >
          <X className="h-4 w-4" />
        </button>
      </div>

      <div className="flex-1 overflow-y-auto px-3 py-2 space-y-1">
        {rootMessage && (
          <MessageBubble
            message={rootMessage}
            currentUserId={currentUserId}
            channelId={channelId}
            usersById={usersById}
            showThreadButton={false}
          />
        )}

        {replies.length > 0 && (
          <div className="my-2 border-t border-gray-100 dark:border-gray-800" />
        )}

        {isLoading ? (
          <div className="flex justify-center py-4">
            <Spinner className="h-5 w-5 text-indigo-600" />
          </div>
        ) : replies.length === 0 ? (
          <p className="py-4 text-center text-xs text-gray-400">No replies yet</p>
        ) : (
          replies.map((msg) => (
            <MessageBubble
              key={msg.id}
              message={msg}
              currentUserId={currentUserId}
              channelId={channelId}
              threadId={threadMessageId}
              usersById={usersById}
              showThreadButton={false}
            />
          ))
        )}
        <div ref={bottomRef} />
      </div>

      <MessageComposer
        channelId={channelId}
        threadId={threadMessageId}
        onTypingStart={() => {}}
        onTypingStop={() => {}}
        placeholder="Reply in thread…"
      />
    </div>
  );
}

// ── MessageComposer ───────────────────────────────────────────────────────────

function MessageComposer({
  channelId,
  threadId,
  onTypingStart,
  onTypingStop,
  placeholder = "Type a message...",
}: {
  channelId: string;
  threadId?: string;
  onTypingStart: () => void;
  onTypingStop: () => void;
  placeholder?: string;
}) {
  const [content, setContent] = useState("");
  const [files, setFiles] = useState<File[]>([]);
  const queryClient = useQueryClient();
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    textareaRef.current?.focus();
  }, [channelId]);

  const mutation = useMutation({
    mutationFn: ({ text, attachedFiles }: { text: string; attachedFiles: File[] }) =>
      sendMessage(channelId, text, threadId, attachedFiles.length > 0 ? attachedFiles : undefined),
    onSuccess: (msg) => {
      setContent("");
      setFiles([]);
      onTypingStop();
      if (threadId) {
        // Thread reply: push to thread cache and bump parent reply count
        queryClient.setQueryData<PaginatedResponse<Message>>(
          ["thread", threadId],
          (old) => {
            if (!old) return { items: [msg], cursor: undefined };
            if (old.items.some((m) => m.id === msg.id)) return old;
            return { ...old, items: [...old.items, msg] };
          },
        );
        queryClient.setQueryData<PaginatedResponse<Message>>(
          ["messages", channelId],
          (old) =>
            old
              ? {
                  ...old,
                  items: old.items.map((m) =>
                    m.id === threadId ? { ...m, replyCount: m.replyCount + 1 } : m,
                  ),
                }
              : old,
        );
      } else {
        // Top-level message: add to channel cache immediately
        const hydrated: Message = {
          ...msg,
          reactions: msg.reactions ?? [],
          attachments: msg.attachments ?? [],
          replyCount: msg.replyCount ?? 0,
        };
        queryClient.setQueryData<PaginatedResponse<Message>>(
          ["messages", channelId],
          (old) => {
            if (!old) return { items: [hydrated], cursor: undefined };
            if (old.items.some((m) => m.id === msg.id)) return old;
            return { ...old, items: [hydrated, ...old.items] };
          },
        );
      }
    },
  });

  const hasContent = content.trim().length > 0 || files.length > 0;

  function handleSubmit(e?: FormEvent) {
    e?.preventDefault();
    if (!hasContent || mutation.isPending) return;
    mutation.mutate({ text: content.trim(), attachedFiles: files });
  }

  function handleKeyDown(e: KeyboardEvent<HTMLTextAreaElement>) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSubmit();
    }
  }

  function handleChange(e: React.ChangeEvent<HTMLTextAreaElement>) {
    setContent(e.target.value);
    if (e.target.value.trim()) {
      onTypingStart();
    } else {
      onTypingStop();
    }
  }

  function handleFileSelect(e: React.ChangeEvent<HTMLInputElement>) {
    const selected = e.target.files;
    if (!selected) return;
    setFiles((prev) => [...prev, ...Array.from(selected)]);
    // Reset so the same file can be re-selected.
    e.target.value = "";
  }

  function removeFile(index: number) {
    setFiles((prev) => prev.filter((_, i) => i !== index));
  }

  function handleDrop(e: React.DragEvent) {
    e.preventDefault();
    const dropped = e.dataTransfer.files;
    if (dropped.length > 0) {
      setFiles((prev) => [...prev, ...Array.from(dropped)]);
    }
  }

  function handleDragOver(e: React.DragEvent) {
    e.preventDefault();
  }

  return (
    <form
      onSubmit={handleSubmit}
      onDrop={handleDrop}
      onDragOver={handleDragOver}
      className="border-t border-gray-200 px-4 py-3 dark:border-gray-700"
    >
      {/* File pills */}
      {files.length > 0 && (
        <div className="mb-2 flex flex-wrap gap-1">
          {files.map((file, i) => (
            <span
              key={`${file.name}-${i}`}
              className="flex items-center gap-1 rounded-full border border-gray-200 bg-gray-50 px-2 py-0.5 text-xs text-gray-700 dark:border-gray-700 dark:bg-gray-800 dark:text-gray-300"
            >
              {file.name}
              <button
                type="button"
                onClick={() => removeFile(i)}
                className="ml-0.5 text-gray-400 hover:text-gray-600 dark:hover:text-gray-200"
              >
                <X className="h-3 w-3" />
              </button>
            </span>
          ))}
        </div>
      )}
      <div className="flex items-end gap-2">
        <button
          type="button"
          onClick={() => fileInputRef.current?.click()}
          className="rounded-md p-2 text-gray-400 hover:bg-gray-100 hover:text-gray-600 dark:hover:bg-gray-700"
          title="Attach files"
        >
          <Paperclip className="h-4 w-4" />
        </button>
        <input
          ref={fileInputRef}
          type="file"
          multiple
          className="hidden"
          onChange={handleFileSelect}
        />
        <textarea
          ref={textareaRef}
          value={content}
          onChange={handleChange}
          onKeyDown={handleKeyDown}
          placeholder={placeholder}
          rows={1}
          className="flex-1 resize-none rounded-md border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 placeholder:text-gray-400 focus:border-indigo-500 focus:outline-none focus:ring-1 focus:ring-indigo-500 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100 dark:placeholder:text-gray-500"
        />
        <button
          type="submit"
          disabled={!hasContent || mutation.isPending}
          className="rounded-md bg-indigo-600 p-2 text-white hover:bg-indigo-500 disabled:opacity-50 disabled:pointer-events-none"
        >
          <Send className="h-4 w-4" />
        </button>
      </div>
    </form>
  );
}

function formatTime(iso: string): string {
  const date = new Date(iso);
  const now = new Date();
  const isToday = date.toDateString() === now.toDateString();

  if (isToday) {
    return date.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
  }

  return date.toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}
