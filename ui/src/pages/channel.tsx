import {
  useState,
  useRef,
  useEffect,
  useCallback,
  useMemo,
} from "react";
import { useParams } from "react-router-dom";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Hash, MessageCircle, Pin } from "lucide-react";
import {
  getChannel,
  listMembers,
  listMessages,
  markChannelRead,
} from "../lib/api/channels";
import { listUsers } from "../lib/api/users";
import { useAuth } from "../lib/auth/context";
import { Spinner } from "../components/ui/spinner";
import { MessageBubble } from "../components/message/message-bubble";
import { MessageComposer } from "../components/message/message-composer";
import { ThreadPanel } from "../components/channel/thread-panel";
import { PinnedMessagesPanel } from "../components/channel/pinned-messages-panel";
import { addReactionLocally, removeReactionLocally } from "../lib/reactions";
import type { Channel, ChannelMember, Message, PaginatedResponse, User } from "../lib/api/types";
import { Virtuoso, type VirtuosoHandle } from "react-virtuoso";
import { useWsEvent, useTypingIndicator } from "../lib/ws/hooks";

export function ChannelPage() {
  const { channelId } = useParams<{ channelId: string }>();
  const { user } = useAuth();
  const queryClient = useQueryClient();
  const [threadMessageId, setThreadMessageId] = useState<string | null>(null);
  const [showPins, setShowPins] = useState(false);

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
  const sorted = useMemo(() => [...messages].reverse(), [messages]);

  const virtuosoRef = useRef<VirtuosoHandle>(null);
  const prevCountRef = useRef(0);
  useEffect(() => {
    if (sorted.length > prevCountRef.current && sorted.length > 0) {
      virtuosoRef.current?.scrollToIndex({ index: sorted.length - 1, behavior: "auto" });
    }
    prevCountRef.current = sorted.length;
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
          <div className="ml-auto">
            <button
              onClick={() => setShowPins((p) => !p)}
              className={`rounded p-1.5 ${showPins ? "bg-indigo-100 text-indigo-600 dark:bg-indigo-900/30 dark:text-indigo-400" : "text-gray-400 hover:bg-gray-100 hover:text-gray-600 dark:hover:bg-gray-800"}`}
              title="Pinned messages"
              aria-label="Toggle pinned messages"
            >
              <Pin className="h-4 w-4" />
            </button>
          </div>
        </header>

        <div className="flex-1 overflow-hidden px-4 py-2" role="log" aria-label="Messages" aria-live="polite">
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
            <Virtuoso
              ref={virtuosoRef}
              data={sorted}
              initialTopMostItemIndex={sorted.length - 1}
              followOutput="smooth"
              itemContent={(_index, msg) => (
                <MessageBubble
                  key={msg.id}
                  message={msg}
                  currentUserId={user?.id ?? ""}
                  channelId={channelId!}
                  usersById={usersById}
                  onOpenThread={() => openThread(msg.id)}
                  isThreadOpen={threadMessageId === msg.id}
                />
              )}
            />
          )}
        </div>

        {typingUsers.size > 0 && (
          <div className="px-4 py-1 text-xs text-gray-400 dark:text-gray-500" aria-live="polite" role="status">
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
            users={usersData?.items ?? []}
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

      {showPins && channelId && !threadMessageId && (
        <PinnedMessagesPanel
          channelId={channelId}
          usersById={usersById}
          onClose={() => setShowPins(false)}
        />
      )}
    </div>
  );
}
