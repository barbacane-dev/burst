import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MessageComposer } from "../message-composer";
import { MAX_FILE_SIZE_BYTES, MAX_FILES_PER_MESSAGE } from "../../../lib/constants";

// Mock the channels API — sendMessage is used by the mutation
vi.mock("../../../lib/api/channels", () => ({
  sendMessage: vi.fn().mockResolvedValue({
    id: "msg_1",
    channelId: "ch_test",
    userId: "usr_test",
    content: "",
    createdAt: "2026-01-01T00:00:00Z",
    reactions: [],
    attachments: [],
    replyCount: 0,
  }),
}));

// Mock the MentionAutocomplete to avoid rendering complexity
vi.mock("../mention-autocomplete", () => ({
  MentionAutocomplete: () => null,
}));

function createFile(name: string, sizeBytes: number): File {
  const buffer = new ArrayBuffer(sizeBytes);
  return new File([buffer], name, { type: "application/octet-stream" });
}

function renderComposer() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });

  return render(
    <QueryClientProvider client={queryClient}>
      <MessageComposer
        channelId="ch_test"
        onTypingStart={vi.fn()}
        onTypingStop={vi.fn()}
      />
    </QueryClientProvider>,
  );
}

function getFileInput(container: HTMLElement): HTMLInputElement {
  // The hidden file input is inside the form
  const input = container.querySelector('input[type="file"]');
  if (!input) throw new Error("File input not found");
  return input as HTMLInputElement;
}

function makeFileList(files: File[]): FileList {
  const list = {
    length: files.length,
    item: (i: number) => files[i] ?? null,
    [Symbol.iterator]: function* () {
      for (const f of files) yield f;
    },
  } as unknown as FileList;
  for (let i = 0; i < files.length; i++) {
    (list as Record<number, File>)[i] = files[i];
  }
  return list;
}

function selectFiles(input: HTMLInputElement, files: File[]) {
  Object.defineProperty(input, "files", {
    value: makeFileList(files),
    configurable: true,
  });
  fireEvent.change(input);
}

describe("MessageComposer file validation", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("rejects a file that exceeds 10 MB and shows error", () => {
    const { container } = renderComposer();
    const input = getFileInput(container);

    const bigFile = createFile("huge.zip", MAX_FILE_SIZE_BYTES + 1);
    selectFiles(input, [bigFile]);

    expect(screen.getByText(/File exceeds 10 MB limit/)).toBeInTheDocument();
    expect(screen.getByText(/huge\.zip/)).toBeInTheDocument();
    // The file should NOT appear as a pill
    expect(screen.queryByText("huge.zip")?.closest("[class*='rounded-full']")).toBeNull;
  });

  it("rejects when adding files would exceed the maximum count", () => {
    const { container } = renderComposer();
    const input = getFileInput(container);

    // First, add exactly MAX_FILES_PER_MESSAGE files
    const batch = Array.from({ length: MAX_FILES_PER_MESSAGE }, (_, i) =>
      createFile(`file-${i}.txt`, 100),
    );
    selectFiles(input, batch);

    // All 10 should be present as pills
    for (let i = 0; i < MAX_FILES_PER_MESSAGE; i++) {
      expect(screen.getByText(`file-${i}.txt`)).toBeInTheDocument();
    }

    // Now try to add one more
    const extraFile = createFile("extra.txt", 100);
    selectFiles(input, [extraFile]);

    expect(
      screen.getByText(`Maximum ${MAX_FILES_PER_MESSAGE} files per message`),
    ).toBeInTheDocument();
    // The extra file should not appear
    expect(screen.queryByText("extra.txt")).not.toBeInTheDocument();
  });

  it("accepts valid files and shows them as pills", () => {
    const { container } = renderComposer();
    const input = getFileInput(container);

    const validFile = createFile("doc.pdf", 1024);
    selectFiles(input, [validFile]);

    expect(screen.getByText("doc.pdf")).toBeInTheDocument();
    // No error should be shown
    expect(screen.queryByText(/File exceeds/)).not.toBeInTheDocument();
    expect(screen.queryByText(/Maximum/)).not.toBeInTheDocument();
  });

  it("shows error message in a visible error banner", () => {
    const { container } = renderComposer();
    const input = getFileInput(container);

    const bigFile = createFile("toobig.bin", MAX_FILE_SIZE_BYTES + 1024);
    selectFiles(input, [bigFile]);

    const errorBanner = screen.getByText(/File exceeds 10 MB limit: toobig\.bin/);
    expect(errorBanner).toBeInTheDocument();
    // Verify it's in the red error container
    expect(errorBanner.closest("div")).toHaveClass("text-red-700");
  });

  it("clears previous error on a new valid file selection", () => {
    const { container } = renderComposer();
    const input = getFileInput(container);

    // First trigger an error
    const bigFile = createFile("large.bin", MAX_FILE_SIZE_BYTES + 1);
    selectFiles(input, [bigFile]);
    expect(screen.getByText(/File exceeds 10 MB limit/)).toBeInTheDocument();

    // Now select a valid file — error should be cleared
    const validFile = createFile("small.txt", 512);
    selectFiles(input, [validFile]);

    expect(screen.queryByText(/File exceeds 10 MB limit/)).not.toBeInTheDocument();
    expect(screen.getByText("small.txt")).toBeInTheDocument();
  });

  it("clears previous error even when new validation fails differently", () => {
    const { container } = renderComposer();
    const input = getFileInput(container);

    // First trigger a size error
    const bigFile = createFile("large.bin", MAX_FILE_SIZE_BYTES + 1);
    selectFiles(input, [bigFile]);
    expect(screen.getByText(/File exceeds 10 MB limit/)).toBeInTheDocument();

    // Fill up to max
    const batch = Array.from({ length: MAX_FILES_PER_MESSAGE }, (_, i) =>
      createFile(`f-${i}.txt`, 100),
    );
    selectFiles(input, batch);

    // Size error should be gone — now we have a count limit
    expect(screen.queryByText(/File exceeds 10 MB limit/)).not.toBeInTheDocument();

    // Try to exceed count
    const extra = createFile("one-more.txt", 100);
    selectFiles(input, [extra]);
    expect(
      screen.getByText(`Maximum ${MAX_FILES_PER_MESSAGE} files per message`),
    ).toBeInTheDocument();
  });
});
