import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";
import { AdminPage } from "../admin";

// Mock auth — integrator role
vi.mock("../../lib/auth/use-auth", () => ({
  useAuth: () => ({
    user: {
      id: "usr_ivy",
      displayName: "Ivy Integrator",
      username: "ivy",
      role: "integrator",
      status: "online",
      isBot: false,
      createdAt: "2026-01-01T00:00:00Z",
    },
    logout: vi.fn(),
  }),
}));

// Mock webhook API
const mockListAllWebhooks = vi.fn().mockResolvedValue({
  items: [
    {
      id: "wh_001",
      channelId: "ch_general",
      kind: "incoming",
      name: "CI Bot",
      isActive: true,
      createdBy: "usr_ivy",
      createdAt: "2026-03-01T00:00:00Z",
    },
    {
      id: "wh_002",
      channelId: "ch_general",
      kind: "outgoing",
      name: "Alerts",
      url: "https://example.com/hook",
      isActive: false,
      createdBy: "usr_ivy",
      createdAt: "2026-03-02T00:00:00Z",
    },
  ],
  cursor: undefined,
});
const mockCreateWebhook = vi.fn().mockResolvedValue({
  id: "wh_003",
  channelId: "ch_general",
  kind: "incoming",
  name: "New Hook",
  isActive: true,
  createdBy: "usr_ivy",
  createdAt: "2026-03-03T00:00:00Z",
  token: "abc123secret",
});
const mockDeleteWebhook = vi.fn().mockResolvedValue(undefined);

vi.mock("../../lib/api/webhooks", () => ({
  listAllWebhooks: (...args: unknown[]) => mockListAllWebhooks(...args),
  createWebhook: (...args: unknown[]) => mockCreateWebhook(...args),
  deleteWebhook: (...args: unknown[]) => mockDeleteWebhook(...args),
  updateWebhook: vi.fn(),
  regenerateToken: vi.fn(),
}));

// Mock bot API
vi.mock("../../lib/api/bots", () => ({
  listBots: vi.fn().mockResolvedValue({ items: [], cursor: undefined }),
  createBot: vi.fn(),
  deleteBot: vi.fn(),
}));

// Mock channels API (for webhook create dropdown)
vi.mock("../../lib/api/channels", () => ({
  listChannels: vi.fn().mockResolvedValue({
    items: [
      {
        id: "ch_general",
        kind: "public",
        name: "general",
        slug: "general",
        createdBy: "usr_alice",
        isArchived: false,
        isReadonly: false,
        unreadCount: 0,
        createdAt: "2026-01-01T00:00:00Z",
        updatedAt: "2026-01-01T00:00:00Z",
      },
    ],
    cursor: undefined,
  }),
  browseChannels: vi.fn().mockResolvedValue({ items: [], cursor: undefined }),
}));

// Mock admin API (not used by integrator, but imported)
vi.mock("../../lib/api/admin", () => ({
  listAdminUsers: vi.fn().mockResolvedValue({ items: [], cursor: undefined }),
  listAdminChannels: vi.fn().mockResolvedValue({ items: [], cursor: undefined }),
  listAuditLog: vi.fn().mockResolvedValue({ items: [], cursor: undefined }),
  updateAdminUser: vi.fn(),
  updateAdminChannel: vi.fn(),
  deleteAdminChannel: vi.fn(),
}));

// Mock emojis API
vi.mock("../../lib/api/emojis", () => ({
  listAdminEmojis: vi.fn().mockResolvedValue({ items: [], cursor: undefined }),
  createEmoji: vi.fn(),
  deleteEmoji: vi.fn(),
}));

function renderAdmin() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={["/admin"]}>
        <AdminPage />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("Admin page — integrator role", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("shows only Webhooks and Bots tabs for integrator", () => {
    renderAdmin();
    expect(screen.getByText("Webhooks")).toBeInTheDocument();
    expect(screen.getByText("Bots")).toBeInTheDocument();
    expect(screen.queryByText("Users")).not.toBeInTheDocument();
    expect(screen.queryByText("Channels")).not.toBeInTheDocument();
    expect(screen.queryByText("Emojis")).not.toBeInTheDocument();
    expect(screen.queryByText("Audit Log")).not.toBeInTheDocument();
  });

  it("defaults to Webhooks tab for integrator", () => {
    renderAdmin();
    // Webhooks tab should be active (has the indigo border)
    const webhooksTab = screen.getByText("Webhooks");
    expect(webhooksTab.closest("button")).toHaveClass("border-indigo-600");
  });

  it("renders webhook list", async () => {
    renderAdmin();
    await waitFor(() => {
      expect(screen.getByText("CI Bot")).toBeInTheDocument();
      expect(screen.getByText("Alerts")).toBeInTheDocument();
    });
    // Check kind badges
    expect(screen.getByText("incoming")).toBeInTheDocument();
    expect(screen.getByText("outgoing")).toBeInTheDocument();
    // Check inactive badge
    expect(screen.getByText("Inactive")).toBeInTheDocument();
  });

  it("shows trigger URL for incoming webhooks", async () => {
    renderAdmin();
    await waitFor(() => {
      expect(screen.getByText(/Trigger:.*wh_001\/trigger/)).toBeInTheDocument();
    });
  });

  it("shows create webhook form", async () => {
    renderAdmin();
    fireEvent.click(screen.getByText("Create Webhook"));
    expect(screen.getByText("New Webhook")).toBeInTheDocument();
    expect(screen.getByText("Select channel...")).toBeInTheDocument();
  });

  it("calls createWebhook API when form is submitted", async () => {
    renderAdmin();
    fireEvent.click(screen.getByText("Create Webhook"));

    // Wait for channels to load in dropdown
    await waitFor(() => {
      expect(screen.getByText("general")).toBeInTheDocument();
    });

    // Fill form
    const selects = document.querySelectorAll("select");
    fireEvent.change(selects[0], { target: { value: "ch_general" } });
    fireEvent.change(selects[1], { target: { value: "incoming" } });

    const nameInput = screen.getByPlaceholderText("CI Notifications");
    fireEvent.change(nameInput, { target: { value: "New Hook" } });

    // Find and click the submit Create button
    const buttons = screen.getAllByRole("button");
    const createBtn = buttons.find((b) => b.textContent === "Create");
    fireEvent.click(createBtn!);

    await waitFor(() => {
      expect(mockCreateWebhook).toHaveBeenCalledWith("ch_general", {
        kind: "incoming",
        name: "New Hook",
        url: undefined,
      });
    });
  });
});

// Admin tab visibility is covered by E2E tests (admin.spec.ts, webhooks.spec.ts)
