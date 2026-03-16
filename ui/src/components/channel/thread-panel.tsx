import { useRef, useEffect } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { X } from "lucide-react";
import { listThreadReplies } from "../../lib/api/channels";
import { Spinner } from "../ui/spinner";
import { MessageBubble } from "../message/message-bubble";
import { MessageComposer } from "../message/message-composer";
import type { Message, PaginatedResponse } from "../../lib/api/types";

export function ThreadPanel({
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
    <aside aria-label="Thread" className="flex w-80 shrink-0 flex-col border-l border-gray-200 bg-white dark:border-gray-700 dark:bg-gray-900">
      <div className="flex h-14 items-center justify-between border-b border-gray-200 px-4 dark:border-gray-700">
        <span className="text-sm font-semibold text-gray-900 dark:text-gray-100">Thread</span>
        <button
          onClick={onClose}
          aria-label="Close thread"
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
    </aside>
  );
}
