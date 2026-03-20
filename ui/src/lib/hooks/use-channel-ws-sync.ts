import { useQueryClient } from "@tanstack/react-query";
import { useWsEvent } from "../ws/hooks";
import { addReactionLocally, removeReactionLocally } from "../reactions";
import type { Message, PaginatedResponse } from "../api/types";

/**
 * Keeps the channel message cache in sync with incoming WebSocket events.
 *
 * Handles: message.created, message.updated, message.deleted,
 * reaction.added, reaction.removed — all scoped to a single channel.
 *
 * Extracted from ChannelPage where these 5 handlers were inline.
 */
export function useChannelWsSync(
  channelId: string | undefined,
  threadMessageId: string | null,
): void {
  const queryClient = useQueryClient();

  useWsEvent<{ type: string; channelId: string; message: Message }>(
    "message.created",
    (ev) => {
      if (ev.channelId !== channelId) return;
      queryClient.setQueryData<PaginatedResponse<Message>>(
        ["messages", channelId],
        (old) => {
          const hydrated: Message = {
            ...ev.message,
            reactions: ev.message.reactions ?? [],
            attachments: ev.message.attachments ?? [],
            replyCount: ev.message.replyCount ?? 0,
          };
          if (!old) return { items: [hydrated], cursor: undefined };
          if (ev.message.threadId) {
            return {
              ...old,
              items: old.items.map((m) =>
                m.id === ev.message.threadId
                  ? { ...m, replyCount: m.replyCount + 1 }
                  : m,
              ),
            };
          }
          const exists = old.items.some((m) => m.id === ev.message.id);
          if (exists) {
            return {
              ...old,
              items: old.items.map((m) =>
                m.id === ev.message.id ? hydrated : m,
              ),
            };
          }
          return { ...old, items: [hydrated, ...old.items] };
        },
      );
      if (ev.message.threadId && ev.message.threadId === threadMessageId) {
        queryClient.setQueryData<PaginatedResponse<Message>>(
          ["thread", threadMessageId],
          (old) => {
            const hydratedReply: Message = {
              ...ev.message,
              reactions: ev.message.reactions ?? [],
              attachments: ev.message.attachments ?? [],
              replyCount: ev.message.replyCount ?? 0,
            };
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
            ? {
                ...old,
                items: old.items.map((m) =>
                  m.id === ev.message.id ? ev.message : m,
                ),
              }
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

  useWsEvent<{
    type: string;
    channelId: string;
    messageId: string;
    emoji: string;
    userId: string;
  }>("reaction.added", (ev) => {
    if (ev.channelId !== channelId) return;
    const updater = (old: PaginatedResponse<Message> | undefined) =>
      old
        ? {
            ...old,
            items: old.items.map((m) =>
              m.id === ev.messageId
                ? addReactionLocally(m, ev.emoji, ev.userId)
                : m,
            ),
          }
        : old;
    queryClient.setQueryData<PaginatedResponse<Message>>(
      ["messages", channelId],
      updater,
    );
    if (threadMessageId) {
      queryClient.setQueryData<PaginatedResponse<Message>>(
        ["thread", threadMessageId],
        updater,
      );
    }
  });

  useWsEvent<{
    type: string;
    channelId: string;
    messageId: string;
    emoji: string;
    userId: string;
  }>("reaction.removed", (ev) => {
    if (ev.channelId !== channelId) return;
    const updater = (old: PaginatedResponse<Message> | undefined) =>
      old
        ? {
            ...old,
            items: old.items.map((m) =>
              m.id === ev.messageId
                ? removeReactionLocally(m, ev.emoji, ev.userId)
                : m,
            ),
          }
        : old;
    queryClient.setQueryData<PaginatedResponse<Message>>(
      ["messages", channelId],
      updater,
    );
    if (threadMessageId) {
      queryClient.setQueryData<PaginatedResponse<Message>>(
        ["thread", threadMessageId],
        updater,
      );
    }
  });
}
