import { useRef, useEffect } from "react";
import { useQuery } from "@tanstack/react-query";
import { emojiImageSrc, listEmojis, type CustomEmoji } from "../../lib/api/emojis";
import type { PaginatedResponse } from "../../lib/api/types";

const UNICODE_EMOJI = ["👍", "👎", "❤️", "😂", "😮", "😢", "🎉", "🚀", "👀", "🔥"];

export function EmojiPickerDropdown({
  onSelect,
  onClose,
}: {
  onSelect: (emoji: string) => void;
  onClose: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null);

  const { data } = useQuery<PaginatedResponse<CustomEmoji>>({
    queryKey: ["emojis"],
    queryFn: () => listEmojis(),
    staleTime: 60_000,
  });

  const customEmojis = data?.items ?? [];

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
      className="absolute right-0 top-8 z-10 rounded-lg border border-gray-200 bg-white p-2 shadow-lg dark:border-gray-700 dark:bg-gray-800"
      style={{ width: "200px", maxHeight: "240px", overflowY: "auto" }}
    >
      {customEmojis.length > 0 && (
        <>
          <p className="mb-1 px-1 text-[10px] font-semibold uppercase tracking-wider text-gray-400">
            Custom
          </p>
          <div className="mb-2 flex flex-wrap gap-1">
            {customEmojis.map((emoji) => (
              <button
                key={emoji.id}
                onClick={() => onSelect(`:${emoji.shortcode}:`)}
                className="rounded p-1 hover:bg-gray-100 dark:hover:bg-gray-700"
                title={`:${emoji.shortcode}:`}
              >
                <img
                  src={emojiImageSrc(emoji)}
                  alt={emoji.shortcode}
                  className="h-5 w-5 object-contain"
                />
              </button>
            ))}
          </div>
        </>
      )}

      <p className="mb-1 px-1 text-[10px] font-semibold uppercase tracking-wider text-gray-400">
        Emoji
      </p>
      <div className="flex flex-wrap gap-1">
        {UNICODE_EMOJI.map((emoji) => (
          <button
            key={emoji}
            onClick={() => onSelect(emoji)}
            className="rounded p-1 text-lg hover:bg-gray-100 dark:hover:bg-gray-700"
          >
            {emoji}
          </button>
        ))}
      </div>
    </div>
  );
}
