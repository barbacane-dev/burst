import { emojiImageSrc } from "../lib/api/emojis";
import { findCustomEmoji, useCustomEmojis } from "../lib/custom-emojis";

/**
 * An emoji as a message or reaction carries it: the image of a known custom
 * emoji for `:shortcode:`, and the value as text otherwise (a Unicode emoji,
 * or a shortcode that names no custom emoji).
 */
export function Emoji({ value, className }: { value: string; className?: string }) {
  const emoji = findCustomEmoji(useCustomEmojis(), value);
  if (!emoji) return <>{value}</>;
  return (
    <img
      src={emojiImageSrc(emoji)}
      alt={value}
      title={value}
      className={className ?? "inline-block h-5 w-5 object-contain align-text-bottom"}
    />
  );
}
