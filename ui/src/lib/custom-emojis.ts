import { createContext, useContext } from "react";
import type { CustomEmoji } from "./api/emojis";

/**
 * A custom emoji shortcode in text, by the rule the server applies on upload:
 * 2 to 32 ASCII letters, digits and underscores, between colons.
 */
export const SHORTCODE_PATTERN = /:([A-Za-z0-9_]{2,32}):/g;
const WHOLE_SHORTCODE = /^:([A-Za-z0-9_]{2,32}):$/;

export type CustomEmojiMap = ReadonlyMap<string, CustomEmoji>;

/** The workspace's custom emojis by shortcode; empty until they are loaded. */
export const CustomEmojisContext = createContext<CustomEmojiMap>(new Map());

export function useCustomEmojis(): CustomEmojiMap {
  return useContext(CustomEmojisContext);
}

/** The custom emoji a value such as `:party_parrot:` names, if there is one. */
export function findCustomEmoji(emojis: CustomEmojiMap, value: string): CustomEmoji | undefined {
  const match = WHOLE_SHORTCODE.exec(value);
  return match ? emojis.get(match[1]) : undefined;
}

interface Node {
  type: string;
  value?: string;
  children?: Node[];
  data?: Record<string, unknown>;
}

/**
 * A remark plugin that marks each `:shortcode:` in message text as a span
 * carrying `data-emoji`, for the renderer to show as an image when the
 * shortcode is a known custom emoji. Text inside links is left alone, and code
 * is never visited, since its content is a `value` rather than `text` children.
 */
export function remarkCustomEmojis() {
  return (tree: Node) => {
    walk(tree);
  };
}

function walk(node: Node) {
  if (!node.children || node.type === "link" || node.type === "linkReference") return;
  const next: Node[] = [];
  for (const child of node.children) {
    if (child.type === "text" && child.value) {
      let last = 0;
      for (const match of child.value.matchAll(SHORTCODE_PATTERN)) {
        const start = match.index ?? 0;
        if (start > last) next.push({ type: "text", value: child.value.slice(last, start) });
        next.push({
          type: "customEmoji",
          children: [{ type: "text", value: match[0] }],
          data: { hName: "span", hProperties: { "data-emoji": match[1] } },
        });
        last = start + match[0].length;
      }
      if (last < child.value.length) next.push({ type: "text", value: child.value.slice(last) });
    } else {
      walk(child);
      next.push(child);
    }
  }
  node.children = next;
}
