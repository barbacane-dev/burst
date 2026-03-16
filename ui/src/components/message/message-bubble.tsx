import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Smile, MessageSquare } from "lucide-react";
import { addReaction, removeReaction } from "../../lib/api/channels";
import { addReactionLocally, removeReactionLocally } from "../../lib/reactions";
import { formatTime } from "../../lib/format";
import { Avatar } from "../ui/avatar";
import { AttachmentPreview } from "../attachment-preview";
import { EmojiPickerDropdown } from "./emoji-picker-dropdown";
import { MarkdownContent } from "./markdown-content";
import { ReactionPill } from "./reaction-pill";
import type { Message, PaginatedResponse } from "../../lib/api/types";

export function MessageBubble({
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
      role="listitem"
      aria-label={`Message from ${displayName}`}
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

        {isDeleted ? (
          <p className="text-sm italic text-gray-400 dark:text-gray-500">
            This message was deleted
          </p>
        ) : (
          <MarkdownContent content={message.content} />
        )}

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

        {/* Thread reply count */}
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
