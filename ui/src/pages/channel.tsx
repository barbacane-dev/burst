import { useState, useRef, useEffect, type FormEvent, type KeyboardEvent } from "react";
import { useParams } from "react-router-dom";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { Hash, Send } from "lucide-react";
import { getChannel, listMessages, sendMessage } from "../lib/api/channels";
import { useAuth } from "../lib/auth/context";
import { Avatar } from "../components/ui/avatar";
import { Spinner } from "../components/ui/spinner";
import type { Channel, Message, PaginatedResponse } from "../lib/api/types";

export function ChannelPage() {
  const { channelId } = useParams<{ channelId: string }>();
  const { user } = useAuth();
  const queryClient = useQueryClient();

  // Try to find the channel in the sidebar's list cache first; if missing
  // (direct navigation / page reload), fetch it individually.
  const cachedChannels = queryClient.getQueryData<PaginatedResponse<Channel>>(["channels"]);
  const cachedChannel = cachedChannels?.items.find((ch) => ch.id === channelId);

  const { data: fetchedChannel } = useQuery<Channel>({
    queryKey: ["channel", channelId],
    queryFn: () => getChannel(channelId!),
    enabled: !!channelId && !cachedChannel,
    staleTime: 30_000,
  });

  const channel = cachedChannel ?? fetchedChannel;

  const { data, isLoading } = useQuery<PaginatedResponse<Message>>({
    queryKey: ["messages", channelId],
    queryFn: () => listMessages(channelId!),
    enabled: !!channelId,
    staleTime: Infinity,
  });

  const messages = data?.items ?? [];
  // Messages come newest-first from API; reverse for display
  const sorted = [...messages].reverse();

  const bottomRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "instant" });
  }, [sorted.length]);

  // Collect unique user IDs for display — for now just use IDs
  // In a full implementation we'd batch-fetch user profiles

  return (
    <div className="flex flex-1 flex-col">
      <header className="flex h-14 items-center border-b border-gray-200 px-4 dark:border-gray-700">
        <Hash className="mr-2 h-5 w-5 text-gray-400" />
        <h2 className="text-base font-semibold text-gray-900 dark:text-gray-100">
          {channel?.name ?? "Loading..."}
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
              <MessageBubble key={msg.id} message={msg} isOwn={msg.userId === user?.id} />
            ))}
            <div ref={bottomRef} />
          </div>
        )}
      </div>

      {channelId && <MessageComposer channelId={channelId} />}
    </div>
  );
}

function MessageBubble({ message, isOwn }: { message: Message; isOwn: boolean }) {
  const isDeleted = !!message.deletedAt;

  return (
    <div className="group flex items-start gap-3 rounded-md px-2 py-1.5 hover:bg-gray-50 dark:hover:bg-gray-900/50">
      <Avatar name={message.userId} size="sm" />
      <div className="min-w-0 flex-1">
        <div className="flex items-baseline gap-2">
          <span className="text-sm font-semibold text-gray-900 dark:text-gray-100">
            {message.userId.replace("usr_", "").slice(0, 8)}
          </span>
          <time className="text-xs text-gray-400 dark:text-gray-500">
            {formatTime(message.createdAt)}
          </time>
          {message.editedAt && (
            <span className="text-xs text-gray-400 dark:text-gray-500">(edited)</span>
          )}
        </div>
        <p className={`text-sm ${isDeleted ? "italic text-gray-400 dark:text-gray-500" : "text-gray-800 dark:text-gray-200"}`}>
          {isDeleted ? "This message was deleted" : message.content}
        </p>
      </div>
    </div>
  );
}

function MessageComposer({ channelId }: { channelId: string }) {
  const [content, setContent] = useState("");
  const queryClient = useQueryClient();
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const mutation = useMutation({
    mutationFn: (text: string) => sendMessage(channelId, text),
    onSuccess: () => {
      setContent("");
      queryClient.invalidateQueries({ queryKey: ["messages", channelId] });
    },
  });

  function handleSubmit(e?: FormEvent) {
    e?.preventDefault();
    const trimmed = content.trim();
    if (!trimmed || mutation.isPending) return;
    mutation.mutate(trimmed);
  }

  function handleKeyDown(e: KeyboardEvent<HTMLTextAreaElement>) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSubmit();
    }
  }

  return (
    <form
      onSubmit={handleSubmit}
      className="border-t border-gray-200 px-4 py-3 dark:border-gray-700"
    >
      <div className="flex items-end gap-2">
        <textarea
          ref={textareaRef}
          value={content}
          onChange={(e) => setContent(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Type a message..."
          rows={1}
          className="flex-1 resize-none rounded-md border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 placeholder:text-gray-400 focus:border-indigo-500 focus:outline-none focus:ring-1 focus:ring-indigo-500 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100 dark:placeholder:text-gray-500"
        />
        <button
          type="submit"
          disabled={!content.trim() || mutation.isPending}
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
