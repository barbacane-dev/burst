/**
 * Mentions in message text, matched by the same rule the server applies in
 * `burst_core::mentions::parse`: an `@` at the start or after a character that
 * is not a letter or digit, followed by letters, digits and underscores. Keep
 * the two in step, or a highlighted mention would notify no one, or the reverse.
 */
export const MENTION_PATTERN = /(?<![\p{L}\p{N}])@([\p{L}\p{N}_]+)/gu;

/** The two mentions that address more than one person. */
export const BROADCASTS = [
  { name: "channel", description: "Notify everyone in this channel" },
  { name: "here", description: "Notify everyone online" },
] as const;

export function isBroadcast(name: string): boolean {
  const lower = name.toLowerCase();
  return BROADCASTS.some((b) => b.name === lower);
}

export interface MentionSpan {
  kind: "text" | "mention";
  value: string;
  /** The name without its `@`, for a mention. */
  name?: string;
}

/** Splits text into plain runs and mentions. */
export function splitMentions(text: string): MentionSpan[] {
  const spans: MentionSpan[] = [];
  let last = 0;
  for (const match of text.matchAll(MENTION_PATTERN)) {
    const start = match.index ?? 0;
    if (start > last) spans.push({ kind: "text", value: text.slice(last, start) });
    spans.push({ kind: "mention", value: match[0], name: match[1] });
    last = start + match[0].length;
  }
  if (last < text.length) spans.push({ kind: "text", value: text.slice(last) });
  return spans;
}

interface Node {
  type: string;
  value?: string;
  children?: Node[];
  data?: Record<string, unknown>;
}

const BROADCAST_CLASS =
  "rounded bg-amber-100 px-0.5 font-medium text-amber-800 dark:bg-amber-900/40 dark:text-amber-300";
const USER_CLASS =
  "rounded bg-indigo-50 px-0.5 font-medium text-indigo-700 dark:bg-indigo-900/40 dark:text-indigo-300";

/**
 * A remark plugin that wraps mentions in a highlighted span. Text inside
 * links is left alone, and code is never visited, since its content is a
 * `value` rather than `text` children.
 */
export function remarkMentions() {
  return (tree: Node) => {
    walk(tree);
  };
}

function walk(node: Node) {
  if (!node.children || node.type === "link" || node.type === "linkReference") return;
  const next: Node[] = [];
  for (const child of node.children) {
    if (child.type === "text" && child.value) {
      for (const span of splitMentions(child.value)) {
        if (span.kind === "text") {
          next.push({ type: "text", value: span.value });
        } else {
          const broadcast = isBroadcast(span.name ?? "");
          next.push({
            type: "mention",
            children: [{ type: "text", value: span.value }],
            data: {
              hName: "span",
              hProperties: {
                className: broadcast ? BROADCAST_CLASS : USER_CLASS,
                "data-mention": span.name,
                "data-broadcast": broadcast ? "true" : undefined,
              },
            },
          });
        }
      }
    } else {
      walk(child);
      next.push(child);
    }
  }
  node.children = next;
}
