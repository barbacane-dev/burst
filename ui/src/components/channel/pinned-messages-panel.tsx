import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { Pin, X } from "lucide-react";
import { listPins, unpinMessage } from "../../lib/api/channels";
import { Spinner } from "../ui/spinner";
import { formatTime } from "../../lib/format";
import type { Message, PaginatedResponse } from "../../lib/api/types";

export function PinnedMessagesPanel({
  channelId,
  usersById,
  onClose,
}: {
  channelId: string;
  usersById: Map<string, string>;
  onClose: () => void;
}) {
  const queryClient = useQueryClient();

  const { data, isLoading } = useQuery<PaginatedResponse<Message>>({
    queryKey: ["pins", channelId],
    queryFn: () => listPins(channelId),
  });
  const pins = data?.items;

  const unpin = useMutation({
    mutationFn: (messageId: string) => unpinMessage(channelId, messageId),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["pins", channelId] }),
  });

  return (
    <aside aria-label="Pinned messages" className="flex w-80 shrink-0 flex-col border-l border-gray-200 bg-white dark:border-gray-700 dark:bg-gray-900">
      <div className="flex h-14 items-center justify-between border-b border-gray-200 px-4 dark:border-gray-700">
        <div className="flex items-center gap-2">
          <Pin className="h-4 w-4 text-gray-400" />
          <span className="text-sm font-semibold text-gray-900 dark:text-gray-100">
            Pinned Messages
          </span>
        </div>
        <button
          onClick={onClose}
          aria-label="Close pinned messages"
          className="rounded p-1 text-gray-400 hover:bg-gray-100 hover:text-gray-600 dark:hover:bg-gray-700"
        >
          <X className="h-4 w-4" />
        </button>
      </div>

      <div className="flex-1 overflow-y-auto p-3 space-y-2">
        {isLoading ? (
          <div className="flex justify-center py-4">
            <Spinner className="h-5 w-5 text-indigo-600" />
          </div>
        ) : !pins || pins.length === 0 ? (
          <p className="py-4 text-center text-xs text-gray-400">No pinned messages</p>
        ) : (
          pins.map((msg) => {
            const displayName =
              usersById.get(msg.userId) ?? msg.userId.replace("usr_", "").slice(0, 8);

            return (
              <div
                key={msg.id}
                className="rounded-md border border-gray-100 bg-gray-50 p-3 dark:border-gray-800 dark:bg-gray-800/50"
              >
                <div className="flex items-baseline justify-between gap-2">
                  <span className="text-xs font-semibold text-gray-900 dark:text-gray-100">
                    {displayName}
                  </span>
                  <button
                    onClick={() => unpin.mutate(msg.id)}
                    className="text-xs text-gray-400 hover:text-red-500"
                    title="Unpin"
                  >
                    Unpin
                  </button>
                </div>
                <p className="mt-1 text-xs text-gray-700 dark:text-gray-300">
                  {msg.content}
                </p>
                <time className="mt-1 block text-[10px] text-gray-400">
                  {formatTime(msg.createdAt)}
                </time>
              </div>
            );
          })
        )}
      </div>
    </aside>
  );
}
