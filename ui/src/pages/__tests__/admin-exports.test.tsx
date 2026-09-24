import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor, within } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ExportsTab } from "../admin";
import { createExport, deleteExport, downloadExport, listExports, type DataExport } from "../../lib/api/exports";
import { formatSize } from "../../lib/api/exports";

vi.mock("../../lib/api/exports", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../lib/api/exports")>();
  return {
    ...actual,
    listExports: vi.fn(),
    createExport: vi.fn(),
    deleteExport: vi.fn(),
    downloadExport: vi.fn(),
  };
});

vi.mock("../../lib/api/admin", () => ({
  listAdminUsers: vi.fn(),
  listAdminChannels: vi.fn().mockResolvedValue({
    items: [{ id: "ch_general", kind: "public", name: "general", isArchived: false, isReadonly: false, createdAt: "2026-01-01T00:00:00Z" }],
  }),
  listAuditLog: vi.fn(),
  updateAdminUser: vi.fn(),
  updateAdminChannel: vi.fn(),
  deleteAdminChannel: vi.fn(),
}));

vi.mock("../../lib/auth/use-auth", () => ({
  useAuth: () => ({ user: { id: "usr_root", role: "admin" } }),
}));

function exp(extra: Partial<DataExport>): DataExport {
  return {
    id: "exp_1",
    scope: "instance",
    status: "completed",
    requestedBy: "usr_root",
    createdAt: "2026-09-24T10:00:00Z",
    ...extra,
  };
}

function renderTab() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={client}>
      <ExportsTab />
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  vi.mocked(listExports).mockReset();
  vi.mocked(createExport).mockReset();
  vi.mocked(deleteExport).mockReset();
  vi.mocked(downloadExport).mockReset();
});

describe("ExportsTab", () => {
  it("starts an instance export by default, or one channel", async () => {
    vi.mocked(listExports).mockResolvedValue({ items: [] });
    vi.mocked(createExport).mockResolvedValue(exp({ status: "pending" }));
    renderTab();

    fireEvent.click(await screen.findByText("Start export"));
    await waitFor(() => expect(createExport).toHaveBeenCalledWith(undefined));

    await screen.findByRole("option", { name: "#general" });
    fireEvent.change(screen.getByLabelText("What to export"), { target: { value: "ch_general" } });
    fireEvent.click(screen.getByText("Start export"));
    await waitFor(() => expect(createExport).toHaveBeenLastCalledWith("ch_general"));
  });

  it("offers download and delete for a finished export only", async () => {
    vi.mocked(listExports).mockResolvedValue({
      items: [
        exp({ id: "exp_done", sizeBytes: 2048 }),
        exp({ id: "exp_busy", status: "running", scope: "channel", channelId: "ch_general" }),
      ],
    });
    vi.mocked(downloadExport).mockResolvedValue(undefined);
    vi.mocked(deleteExport).mockResolvedValue(undefined);
    renderTab();

    const [done, busy] = await screen.findAllByRole("listitem");
    expect(within(done).getByText(/2.0 KB/)).toBeTruthy();
    expect(within(busy).getByText("Building")).toBeTruthy();
    expect(await within(busy).findByText("#general")).toBeTruthy();
    expect(within(busy).queryByText("Download")).toBeNull();
    expect(within(busy).queryByText("Delete")).toBeNull();

    fireEvent.click(within(done).getByText("Download"));
    await waitFor(() => expect(downloadExport).toHaveBeenCalledWith("exp_done"));
    fireEvent.click(within(done).getByText("Delete"));
    await waitFor(() => expect(deleteExport).toHaveBeenCalledWith("exp_done"));
  });

  it("cannot start a second export while one is in progress", async () => {
    vi.mocked(listExports).mockResolvedValue({ items: [exp({ status: "pending" })] });
    renderTab();
    const button = await screen.findByText("Export in progress");
    expect((button as HTMLButtonElement).disabled).toBe(true);
  });

  it("shows why an export failed", async () => {
    vi.mocked(listExports).mockResolvedValue({ items: [exp({ status: "failed", error: "interrupted" })] });
    renderTab();
    expect(await screen.findByText(/interrupted/)).toBeTruthy();
    expect(screen.getByText("Failed")).toBeTruthy();
  });

  it("follows an export until it finishes", async () => {
    vi.mocked(listExports)
      .mockResolvedValueOnce({ items: [exp({ status: "running" })] })
      .mockResolvedValue({ items: [exp({ status: "completed" })] });
    renderTab();
    expect(await screen.findByText("Building")).toBeTruthy();
    expect(await screen.findByText("Ready", {}, { timeout: 4000 })).toBeTruthy();
  });
});

describe("formatSize", () => {
  it("scales units", () => {
    expect(formatSize(512)).toBe("512 B");
    expect(formatSize(1536)).toBe("1.5 KB");
    expect(formatSize(5 * 1024 ** 3)).toBe("5.0 GB");
  });
});
