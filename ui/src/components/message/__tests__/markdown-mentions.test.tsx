import { describe, it, expect } from "vitest";
import { render } from "@testing-library/react";
import { MarkdownContent } from "../markdown-content";

const names = (content: string) => mentions(content).map((m) => m.dataset.mention);

function mentions(content: string) {
  const { container } = render(<MarkdownContent content={content} />);
  return Array.from(container.querySelectorAll<HTMLElement>("[data-mention]"));
}

describe("mentions are highlighted in rendered messages", () => {
  it("highlights a broadcast as one", () => {
    const [m] = mentions("@channel standup in five");
    expect(m.dataset.mention).toBe("channel");
    expect(m.dataset.broadcast).toBe("true");
    expect(m.textContent).toBe("@channel");
  });

  it("highlights a user mention without marking it a broadcast", () => {
    const [m] = mentions("thanks @bob");
    expect(m.dataset.mention).toBe("bob");
    expect(m.dataset.broadcast).toBeUndefined();
  });

  it("finds a mention nested in formatting", () => {
    expect(mentions("**@here** now").map((m) => m.dataset.mention)).toEqual(["here"]);
  });

  // Each "leaves X alone" case carries a real mention too: with highlighting
  // switched off, "no mention found" would hold trivially.
  it("leaves an email address alone", () => {
    expect(names("mail alice@example.com or ask @bob")).toEqual(["bob"]);
  });

  it("leaves code alone", () => {
    expect(names("run `ping @bob`, ask @carol\n\n```\n@channel\n```")).toEqual(["carol"]);
  });

  it("leaves link text alone", () => {
    expect(names("[@bob](https://example.com) then @carol")).toEqual(["carol"]);
  });

  it("keeps the text around a mention", () => {
    const { container } = render(<MarkdownContent content="hi @bob, ok" />);
    expect(container.querySelector("[data-mention]")?.textContent).toBe("@bob");
    expect(container.textContent).toBe("hi @bob, ok");
  });
});
