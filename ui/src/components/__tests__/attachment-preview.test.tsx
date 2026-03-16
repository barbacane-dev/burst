import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { AttachmentPreview } from "../attachment-preview";
import type { Attachment } from "../../lib/api/types";
import * as client from "../../lib/api/client";

function makeAttachment(overrides: Partial<Attachment> = {}): Attachment {
  return {
    id: "att-1",
    fileName: "photo.png",
    fileSize: 12345,
    contentType: "image/png",
    metadata: {},
    createdAt: "2026-01-01T00:00:00Z",
    ...overrides,
  };
}

describe("AttachmentPreview", () => {
  beforeEach(() => {
    vi.spyOn(client, "getAccessToken").mockReturnValue(null);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("renders image preview for image attachment", () => {
    const attachment = makeAttachment({
      contentType: "image/png",
      fileName: "screenshot.png",
    });

    render(<AttachmentPreview attachment={attachment} />);

    const img = screen.getByRole("img");
    expect(img).toBeInTheDocument();
    expect(img).toHaveAttribute("alt", "screenshot.png");
    expect(img).toHaveAttribute("src", `/api/attachments/${attachment.id}`);
  });

  it("renders file card for non-image attachment", () => {
    const attachment = makeAttachment({
      contentType: "text/plain",
      fileName: "readme.txt",
      fileSize: 2048,
    });

    render(<AttachmentPreview attachment={attachment} />);

    expect(screen.queryByRole("img")).not.toBeInTheDocument();
    expect(screen.getByText("readme.txt")).toBeInTheDocument();
    expect(screen.getByText("2.0 KB")).toBeInTheDocument();
  });

  it("formats file size correctly", () => {
    // Bytes
    const { unmount: u1 } = render(
      <AttachmentPreview attachment={makeAttachment({ fileSize: 512, contentType: "text/plain", fileName: "small.txt" })} />,
    );
    expect(screen.getByText("512 B")).toBeInTheDocument();
    u1();

    // Kilobytes
    const { unmount: u2 } = render(
      <AttachmentPreview attachment={makeAttachment({ fileSize: 1536, contentType: "text/plain", fileName: "medium.txt" })} />,
    );
    expect(screen.getByText("1.5 KB")).toBeInTheDocument();
    u2();

    // Megabytes
    render(
      <AttachmentPreview attachment={makeAttachment({ fileSize: 5242880, contentType: "text/plain", fileName: "large.zip" })} />,
    );
    expect(screen.getByText("5.0 MB")).toBeInTheDocument();
  });

  it("uses width/height from metadata for images", () => {
    const attachment = makeAttachment({
      metadata: { width: 800, height: 600 },
    });

    render(<AttachmentPreview attachment={attachment} />);

    const img = screen.getByRole("img");
    expect(img).toHaveAttribute("width", "800");
    expect(img).toHaveAttribute("height", "600");
  });

  it("download link points to correct URL", () => {
    const attachment = makeAttachment({
      id: "att-42",
      contentType: "application/pdf",
      fileName: "report.pdf",
    });

    render(<AttachmentPreview attachment={attachment} />);

    const link = screen.getByRole("link");
    expect(link).toHaveAttribute("href", "/api/attachments/att-42");
  });
});
