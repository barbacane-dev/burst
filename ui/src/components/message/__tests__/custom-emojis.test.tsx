import { afterEach, describe, expect, it } from "vitest";
import { render } from "@testing-library/react";
import type { ReactNode } from "react";
import { MarkdownContent } from "../markdown-content";
import { ReactionPill } from "../reaction-pill";
import { CustomEmojisContext, type CustomEmojiMap } from "../../../lib/custom-emojis";
import { setAccessToken } from "../../../lib/api/client";

const parrot = {
  id: "0193a5e0-7c1a-7000-8000-000000000300",
  shortcode: "party_parrot",
  imageUrl: "/api/emojis/0193a5e0-7c1a-7000-8000-000000000300/image",
  createdBy: "usr_x",
  createdAt: "2026-09-28T00:00:00Z",
};
const emojis: CustomEmojiMap = new Map([[parrot.shortcode, parrot]]);

function withEmojis(ui: ReactNode) {
  return render(<CustomEmojisContext.Provider value={emojis}>{ui}</CustomEmojisContext.Provider>);
}

const images = (container: HTMLElement) =>
  Array.from(container.querySelectorAll("img")).map((img) => img.getAttribute("alt"));

afterEach(() => setAccessToken(null));

describe("custom emojis in message text", () => {
  it("shows a known shortcode as its image, with the token in the URL", () => {
    setAccessToken("tok");
    const { container } = withEmojis(<MarkdownContent content="ship it :party_parrot: now" />);
    const img = container.querySelector("img");
    expect(img?.getAttribute("alt")).toBe(":party_parrot:");
    expect(img?.getAttribute("src")).toBe(`${parrot.imageUrl}?access_token=tok`);
    expect(container.textContent).toContain("ship it");
  });

  it("leaves an unknown shortcode as text", () => {
    const { container } = withEmojis(<MarkdownContent content=":nope: and :party_parrot:" />);
    expect(images(container)).toEqual([":party_parrot:"]);
    expect(container.textContent).toContain(":nope:");
  });

  // Each "leaves X alone" case carries a real emoji too, so it fails if the
  // plugin is off rather than passing trivially.
  it("leaves code alone", () => {
    const { container } = withEmojis(
      <MarkdownContent content={"`:party_parrot:` then :party_parrot:\n\n```\n:party_parrot:\n```"} />,
    );
    expect(images(container)).toEqual([":party_parrot:"]);
  });

  it("leaves link text alone", () => {
    const { container } = withEmojis(
      <MarkdownContent content="[:party_parrot:](https://example.com) :party_parrot:" />,
    );
    expect(images(container)).toEqual([":party_parrot:"]);
  });

  it("keeps mentions highlighted next to an emoji", () => {
    const { container } = withEmojis(<MarkdownContent content="@bob :party_parrot:" />);
    expect(container.querySelector("[data-mention]")?.getAttribute("data-mention")).toBe("bob");
    expect(images(container)).toEqual([":party_parrot:"]);
  });

  it("shows text when the emojis are not loaded", () => {
    const { container } = render(<MarkdownContent content=":party_parrot:" />);
    expect(container.querySelector("img")).toBeNull();
    expect(container.textContent).toBe(":party_parrot:");
  });
});

describe("custom emojis in reactions", () => {
  const pill = (emoji: string) =>
    withEmojis(
      <ReactionPill
        reaction={{ emoji, count: 1, userIds: ["usr_a"] }}
        currentUserId="usr_a"
        onClick={() => {}}
      />,
    );

  it("shows a custom emoji reaction as its image", () => {
    const { container } = pill(":party_parrot:");
    expect(images(container)).toEqual([":party_parrot:"]);
    expect(container.textContent).toBe("1");
  });

  it("shows a Unicode reaction as text", () => {
    const { container } = pill("🚀");
    expect(container.querySelector("img")).toBeNull();
    expect(container.textContent).toBe("🚀1");
  });
});
