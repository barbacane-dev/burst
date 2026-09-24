import { describe, it, expect } from "vitest";
import { splitMentions, isBroadcast } from "../mentions";

const names = (text: string) =>
  splitMentions(text)
    .filter((s) => s.kind === "mention")
    .map((s) => s.name);

// Mirrors the cases in `burst_core::mentions`, so the highlight agrees with
// who the server notifies.
describe("mention matching agrees with the server", () => {
  it("finds a mention", () => {
    expect(names("hey @bob check this")).toEqual(["bob"]);
  });

  it("does not treat an email address as a mention", () => {
    expect(names("write to alice@example.com")).toEqual([]);
  });

  it("finds a mention at the start and after punctuation", () => {
    expect(names("@alice, (@bob)")).toEqual(["alice", "bob"]);
  });

  it("does not treat a bare @ as a mention", () => {
    expect(names("meet @ noon")).toEqual([]);
  });

  it("keeps underscores in the name", () => {
    expect(names("@dev_ops")).toEqual(["dev_ops"]);
  });

  it("matches letters beyond ASCII, as Rust's is_alphanumeric does", () => {
    expect(names("merci @élodie")).toEqual(["élodie"]);
  });

  it("preserves the surrounding text", () => {
    expect(splitMentions("a @b c").map((s) => s.value).join("")).toBe("a @b c");
  });
});

describe("broadcasts", () => {
  it("recognises @channel and @here in any case", () => {
    expect(isBroadcast("channel")).toBe(true);
    expect(isBroadcast("HERE")).toBe(true);
  });

  it("does not treat a longer name as a broadcast", () => {
    expect(isBroadcast("channels")).toBe(false);
    expect(isBroadcast("hereford")).toBe(false);
  });
});
