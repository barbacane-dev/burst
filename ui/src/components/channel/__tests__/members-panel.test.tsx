import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor, within } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MembersPanel } from "../members-panel";
import * as api from "../../../lib/api/channels";
import type { Channel, ChannelMember, User } from "../../../lib/api/types";

const { navigateSpy } = vi.hoisted(() => ({ navigateSpy: vi.fn() }));
vi.mock("react-router-dom", async (importOriginal) => {
  const actual = await importOriginal<typeof import("react-router-dom")>();
  return { ...actual, useNavigate: () => navigateSpy };
});

vi.mock("../../../lib/api/channels", () => ({
  listMembers: vi.fn(),
  addMember: vi.fn().mockResolvedValue(undefined),
  removeMember: vi.fn().mockResolvedValue(undefined),
  setMemberRole: vi.fn().mockResolvedValue({}),
  leaveChannel: vi.fn().mockResolvedValue(undefined),
  archiveChannel: vi.fn().mockResolvedValue({}),
  unarchiveChannel: vi.fn().mockResolvedValue({}),
}));

function user(id: string, name: string, role: User["role"] = "member"): User {
  return { id, username: name.toLowerCase(), displayName: name, role, status: "online", isBot: false, createdAt: "" };
}

const alice = user("usr_alice", "Alice");
const bob = user("usr_bob", "Bob");

const quietBob = {
  ...user("usr_quiet", "Quiet Bob"),
  doNotDisturbUntil: new Date(Date.now() + 60_000).toISOString(),
};
const mo = user("usr_mo", "Mo");
const gus = user("usr_gus", "Gus", "guest");
const dana = user("usr_dana", "Dana");
const everyone = [alice, bob, quietBob, mo, gus, dana];

const members: ChannelMember[] = [
  { userId: "usr_alice", role: "owner", joinedAt: "" },
  { userId: "usr_mo", role: "moderator", joinedAt: "" },
  { userId: "usr_bob", role: "member", joinedAt: "" },
  { userId: "usr_quiet", role: "member", joinedAt: "" },
  { userId: "usr_gus", role: "member", joinedAt: "" },
];

function channel(kind: Channel["kind"] = "private"): Channel {
  return { id: "ch_room", kind, name: "room", createdBy: "usr_alice", isArchived: false, isReadonly: false, createdAt: "", updatedAt: "" } as Channel;
}

async function open(as: User, kind: Channel["kind"] = "private") {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <MembersPanel channel={channel(kind)} currentUser={as} users={everyone} onClose={vi.fn()} />
      </MemoryRouter>
    </QueryClientProvider>,
  );
  await screen.findByText(/Members \(5\)/);
}

const q = (name: string) => screen.queryByRole("button", { name });

describe("the members panel offers what the caller may do", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(api.listMembers).mockResolvedValue(members);
  });

  it("lets the owner add people, appoint moderators and remove members", async () => {
    await open(alice);
    expect(screen.getByText("Add people")).toBeTruthy();
    expect(q("Make Bob a moderator")).toBeTruthy();
    expect(q("Make Mo a member")).toBeTruthy();
    expect(q("Remove Bob")).toBeTruthy();
    expect(q("Remove Mo")).toBeTruthy();
    expect(screen.getByText("Archive channel")).toBeTruthy();
  });

  it("never offers anything against the caller themselves", async () => {
    await open(alice);
    expect(q("Remove Alice")).toBeNull();
    expect(q("Make Alice a moderator")).toBeNull();
  });

  it("lets a moderator remove members but not other moderators, nor appoint", async () => {
    await open(mo);
    expect(q("Remove Bob")).toBeTruthy();
    expect(q("Remove Alice")).toBeNull();
    expect(q("Make Bob a moderator")).toBeNull();
    expect(screen.getByText("Archive channel")).toBeTruthy();
  });

  it("lets a member add people and leave, and nothing more", async () => {
    await open(bob);
    expect(screen.getByText("Add people")).toBeTruthy();
    expect(q("Remove Gus")).toBeNull();
    expect(q("Make Gus a moderator")).toBeNull();
    expect(screen.queryByText("Archive channel")).toBeNull();
    expect(screen.getByText("Leave channel")).toBeTruthy();
  });

  it("offers a guest nothing but leaving", async () => {
    await open(gus);
    expect(screen.queryByText("Add people")).toBeNull();
    expect(q("Remove Bob")).toBeNull();
    expect(screen.queryByText("Archive channel")).toBeNull();
    expect(screen.getByText("Leave channel")).toBeTruthy();
  });

  it("offers no membership controls on a direct message", async () => {
    await open(alice, "group_dm");
    expect(screen.queryByText("Add people")).toBeNull();
    expect(q("Remove Bob")).toBeNull();
    expect(screen.queryByText("Leave channel")).toBeNull();
  });
});

describe("the members panel acts through the API", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(api.listMembers).mockResolvedValue(members);
  });

  it("adds someone who is not yet a member", async () => {
    await open(bob);
    fireEvent.click(screen.getByText("Add people"));
    expect(q("Add Bob")).toBeNull(); // already a member
    fireEvent.click(screen.getByRole("button", { name: "Add Dana" }));
    await waitFor(() => expect(api.addMember).toHaveBeenCalledWith("ch_room", "usr_dana"));
  });

  it("removes a member, appoints a moderator, and leaves", async () => {
    await open(alice);
    fireEvent.click(screen.getByRole("button", { name: "Remove Bob" }));
    await waitFor(() => expect(api.removeMember).toHaveBeenCalledWith("ch_room", "usr_bob"));

    fireEvent.click(screen.getByRole("button", { name: "Make Bob a moderator" }));
    await waitFor(() => expect(api.setMemberRole).toHaveBeenCalledWith("ch_room", "usr_bob", "moderator"));

    fireEvent.click(screen.getByText("Leave channel"));
    await waitFor(() => expect(navigateSpy).toHaveBeenCalledWith("/"));
  });

  it("shows why an action failed", async () => {
    vi.mocked(api.removeMember).mockRejectedValueOnce(new Error("Insufficient permissions"));
    await open(alice);
    fireEvent.click(screen.getByRole("button", { name: "Remove Bob" }));
    expect(await screen.findByRole("alert")).toHaveProperty("textContent", "Insufficient permissions");
  });
});

describe("do-not-disturb indicators", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(api.listMembers).mockResolvedValue(members);
  });

  it("shows the do-not-disturb indicator for quiet members", async () => {
    await open(alice);

    const quietMember = screen.getByText("Quiet Bob", { exact: true }).closest("li");
    const normalMember = screen.getByText("Bob", { exact: true }).closest("li");

    expect(quietMember).not.toBeNull();
    expect(normalMember).not.toBeNull();

    expect(within(quietMember!).getByRole("img", { name: /do not disturb until/i })).toBeTruthy();
    expect(within(normalMember!).queryByRole("img", { name: /do not disturb until/i })).toBeNull();
  });
});

