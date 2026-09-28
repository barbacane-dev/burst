import { afterEach, describe, expect, it } from "vitest";
import { setAccessToken } from "../client";
import { emojiImageSrc } from "../emojis";

describe("emojiImageSrc", () => {
  afterEach(() => setAccessToken(null));

  const emoji = { imageUrl: "/api/emojis/0193a5e0-7c1a-7000-8000-000000000300/image" };

  it("loads the image from the URL the API gives, with the token as a query parameter", () => {
    setAccessToken("tok/en+1");
    expect(emojiImageSrc(emoji)).toBe(
      "/api/emojis/0193a5e0-7c1a-7000-8000-000000000300/image?access_token=tok%2Fen%2B1",
    );
  });

  it("uses the URL as it is when signed out", () => {
    expect(emojiImageSrc(emoji)).toBe(emoji.imageUrl);
  });
});
