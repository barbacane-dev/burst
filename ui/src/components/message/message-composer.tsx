import {
  useState,
  useRef,
  useEffect,
  type FormEvent,
  type KeyboardEvent,
} from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Send, X, Paperclip } from "lucide-react";
import { sendMessage } from "../../lib/api/channels";
import { MAX_FILE_SIZE_BYTES, MAX_FILES_PER_MESSAGE } from "../../lib/constants";
import { MentionAutocomplete } from "./mention-autocomplete";
import type { Message, PaginatedResponse, User } from "../../lib/api/types";

export function MessageComposer({
  channelId,
  threadId,
  onTypingStart,
  onTypingStop,
  placeholder = "Type a message...",
  users = [],
}: {
  channelId: string;
  threadId?: string;
  onTypingStart: () => void;
  onTypingStop: () => void;
  placeholder?: string;
  users?: User[];
}) {
  const [content, setContent] = useState("");
  const [mentionQuery, setMentionQuery] = useState<string | null>(null);
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
    if (e.key === "Escape" && mentionQuery !== null) {
      setMentionQuery(null);
      return;
    }
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      setMentionQuery(null);
      handleSubmit();
    }
  }

  function handleChange(e: React.ChangeEvent<HTMLTextAreaElement>) {
    const val = e.target.value;
    setContent(val);
    if (val.trim()) {
      onTypingStart();
    } else {
      onTypingStop();
    }

    // Detect @mention trigger
    const cursor = e.target.selectionStart ?? val.length;
    const before = val.slice(0, cursor);
    const match = before.match(/@(\w*)$/);
    setMentionQuery(match ? match[1] : null);
  }

  function insertMention(username: string) {
    const cursor = textareaRef.current?.selectionStart ?? content.length;
    const before = content.slice(0, cursor);
    const after = content.slice(cursor);
    const replaced = before.replace(/@\w*$/, `@${username} `);
    setContent(replaced + after);
    setMentionQuery(null);
    textareaRef.current?.focus();
  }

  const [fileError, setFileError] = useState<string | null>(null);

  function validateAndAddFiles(incoming: File[]) {
    setFileError(null);
    const tooLarge = incoming.filter((f) => f.size > MAX_FILE_SIZE_BYTES);
    if (tooLarge.length > 0) {
      setFileError(`File exceeds 10 MB limit: ${tooLarge.map((f) => f.name).join(", ")}`);
      return;
    }
    setFiles((prev) => {
      const combined = [...prev, ...incoming];
      if (combined.length > MAX_FILES_PER_MESSAGE) {
        setFileError(`Maximum ${MAX_FILES_PER_MESSAGE} files per message`);
        return prev;
      }
      return combined;
    });
  }

  function handleFileSelect(e: React.ChangeEvent<HTMLInputElement>) {
    const selected = e.target.files;
    if (!selected) return;
    validateAndAddFiles(Array.from(selected));
    e.target.value = "";
  }

  function removeFile(index: number) {
    setFiles((prev) => prev.filter((_, i) => i !== index));
  }

  function handleDrop(e: React.DragEvent) {
    e.preventDefault();
    const dropped = e.dataTransfer.files;
    if (dropped.length > 0) {
      validateAndAddFiles(Array.from(dropped));
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
      {fileError && (
        <div className="mb-2 rounded-md bg-red-50 px-3 py-1.5 text-xs text-red-700 dark:bg-red-900/30 dark:text-red-400">
          {fileError}
        </div>
      )}
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
      <div className="relative flex items-end gap-2">
        {mentionQuery !== null && (
          <MentionAutocomplete
            query={mentionQuery}
            users={users}
            onSelect={insertMention}
            onClose={() => setMentionQuery(null)}
          />
        )}
        <button
          type="button"
          onClick={() => fileInputRef.current?.click()}
          className="rounded-md p-2 text-gray-400 hover:bg-gray-100 hover:text-gray-600 dark:hover:bg-gray-700"
          title="Attach files"
          aria-label="Attach files"
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
          aria-label="Message input"
          rows={1}
          className="flex-1 resize-none rounded-md border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 placeholder:text-gray-400 focus:border-indigo-500 focus:outline-none focus:ring-1 focus:ring-indigo-500 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100 dark:placeholder:text-gray-500"
        />
        <button
          type="submit"
          disabled={!hasContent || mutation.isPending}
          aria-label="Send message"
          className="rounded-md bg-indigo-600 p-2 text-white hover:bg-indigo-500 disabled:opacity-50 disabled:pointer-events-none"
        >
          <Send className="h-4 w-4" />
        </button>
      </div>
    </form>
  );
}
