import { useRef, useEffect } from "react";

const EMOJI_PICKER = ["👍", "👎", "❤️", "😂", "😮", "😢", "🎉", "🚀", "👀", "🔥"];

export function EmojiPickerDropdown({
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
