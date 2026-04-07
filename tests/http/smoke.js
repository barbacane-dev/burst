/**
 * Burst API — End-to-end smoke tests (user perspective)
 *
 * Exercises the full stack: mock OIDC → Barbacane gateway → Burst API → PostgreSQL
 *
 * Prerequisites:
 *   make services          # PostgreSQL + mock OIDC (Docker)
 *   make seed              # Seed alice + bob users
 *   make server            # Burst API on :3000
 *   make gateway           # Barbacane gateway on :8080
 *
 * Usage:
 *   k6 run tests/http/smoke.js
 */

import http from "k6/http";
import { check, group } from "k6";
import { sha256 } from "k6/crypto";

const GATEWAY = __ENV.GATEWAY_URL || "http://localhost:8080";
const MOCK_OAUTH = __ENV.MOCK_OAUTH_URL || "http://localhost:9099";
const BURST = __ENV.BURST_URL || "http://localhost:3000";

// Pre-compute SHA-256 hashes of fixture files at init time
const HELLO_TXT = open("./fixtures/hello.txt");
const SMALL_PNG = open("./fixtures/Digital_punk_pirate_avatar_small.png", "b");
const LARGE_PNG = open("./fixtures/Digital_punk_pirate_avatar.png", "b");

export const options = {
  scenarios: {
    smoke: {
      executor: "per-vu-iterations",
      vus: 1,
      iterations: 1,
      exec: "smoke",
    },
    rateLimit: {
      executor: "per-vu-iterations",
      vus: 1,
      iterations: 1,
      exec: "rateLimit",
      // Start after the smoke scenario finishes to avoid polluting its
      // rate-limit window (global quota is 300 req / 60 s per consumer).
      startTime: "10s",
    },
  },
  thresholds: {
    checks: ["rate==1.0"], // All checks must pass
  },
};

// ── Helpers ──────────────────────────────────────────────────────────────────

function authHeaders(token) {
  return {
    headers: {
      Authorization: `Bearer ${token}`,
      Accept: "application/json",
    },
  };
}

function jsonAuthHeaders(token) {
  return {
    headers: {
      Authorization: `Bearer ${token}`,
      Accept: "application/json",
      "Content-Type": "application/json",
    },
  };
}

function getToken(username) {
  const res = http.post(
    `${MOCK_OAUTH}/burst/token`,
    `grant_type=password&username=${username}&password=secret&client_id=burst&scope=openid`,
    { headers: { "Content-Type": "application/x-www-form-urlencoded" } },
  );
  check(res, { [`${username} token → 200`]: (r) => r.status === 200 });
  const body = res.json();
  check(body, {
    [`${username} access_token present`]: (b) =>
      typeof b.access_token === "string" && b.access_token.length > 0,
  });
  return body.access_token;
}

// ── Main scenario ────────────────────────────────────────────────────────────

export function smoke() {
  // ── 0. Health Checks ─────────────────────────────────────────────────────
  group("0. Health Checks", () => {
    const oidc = http.get(
      `${MOCK_OAUTH}/burst/.well-known/openid-configuration`,
      { headers: { Accept: "application/json" } },
    );
    check(oidc, {
      "OIDC discovery → 200": (r) => r.status === 200,
      "Issuer present": (r) => r.json().issuer !== undefined,
    });

    const burst = http.get(`${BURST}/api/channels`, {
      headers: { Accept: "application/json" },
    });
    check(burst, {
      "Burst server reachable (401)": (r) => r.status === 401,
    });
  });

  // ── 1. Authentication ────────────────────────────────────────────────────
  let aliceToken, bobToken;
  group("1. Authentication", () => {
    aliceToken = getToken("alice");
    bobToken = getToken("bob");
  });

  // ── Setup: clean up leftover smoke-test channel from previous runs ──────
  group("Setup: idempotent cleanup", () => {
    const list = http.get(`${GATEWAY}/api/admin/channels`, authHeaders(aliceToken));
    if (list.status === 200) {
      const channels = list.json().items || [];
      const stale = channels.find((ch) => ch.slug === "smoke-test");
      if (stale) {
        const del = http.del(
          `${GATEWAY}/api/admin/channels/${stale.id}`,
          null,
          authHeaders(aliceToken),
        );
        check(del, { "Cleanup stale smoke-test channel → 204": (r) => r.status === 204 });
      }
    }
  });

  // ── 2. Gateway Auth Validation ───────────────────────────────────────────
  group("2. Gateway Auth", () => {
    const noAuth = http.get(`${GATEWAY}/api/channels`, {
      headers: { Accept: "application/json" },
    });
    check(noAuth, { "No auth → 401": (r) => r.status === 401 });

    const badAuth = http.get(`${GATEWAY}/api/channels`, {
      headers: { Accept: "application/json", Authorization: "Bearer bad-jwt" },
    });
    check(badAuth, { "Bad token → 401": (r) => r.status === 401 });

    const goodAuth = http.get(`${GATEWAY}/api/channels`, authHeaders(aliceToken));
    check(goodAuth, {
      "Valid token passes gateway → 200": (r) => r.status === 200,
    });
  });

  // ── 3. Channels CRUD ────────────────────────────────────────────────────
  let channelId;
  group("3. Channels CRUD", () => {
    const list = http.get(`${GATEWAY}/api/channels`, authHeaders(aliceToken));
    check(list, {
      "List channels → 200": (r) => r.status === 200,
      "Has items array": (r) => Array.isArray(r.json().items),
    });

    const create = http.post(
      `${GATEWAY}/api/channels`,
      JSON.stringify({
        name: "Smoke Test",
        slug: "smoke-test",
        kind: "public",
        topic: "Smoke testing",
        description: "Created by k6",
      }),
      jsonAuthHeaders(aliceToken),
    );
    check(create, {
      "Create channel → 201": (r) => r.status === 201,
      "Name matches": (r) => r.json().name === "Smoke Test",
      "Slug matches": (r) => r.json().slug === "smoke-test",
    });
    channelId = create.json().id;

    const get = http.get(
      `${GATEWAY}/api/channels/${channelId}`,
      authHeaders(aliceToken),
    );
    check(get, { "Get channel by ID → 200": (r) => r.status === 200 });

    const dup = http.post(
      `${GATEWAY}/api/channels`,
      JSON.stringify({ name: "Dup", slug: "smoke-test", kind: "public" }),
      jsonAuthHeaders(aliceToken),
    );
    check(dup, { "Duplicate slug → 409": (r) => r.status === 409 });

    const patch = http.patch(
      `${GATEWAY}/api/channels/${channelId}`,
      JSON.stringify({ topic: "Smoke testing (updated)" }),
      jsonAuthHeaders(aliceToken),
    );
    check(patch, {
      "Update channel → 200": (r) => r.status === 200,
      "Topic updated": (r) =>
        r.json().topic === "Smoke testing (updated)",
    });
  });

  // ── 4. Membership ───────────────────────────────────────────────────────
  group("4. Membership", () => {
    const join = http.post(
      `${GATEWAY}/api/channels/${channelId}/members`,
      null,
      authHeaders(bobToken),
    );
    check(join, { "Bob joins channel → 204": (r) => r.status === 204 });

    const members = http.get(
      `${GATEWAY}/api/channels/${channelId}/members`,
      authHeaders(aliceToken),
    );
    check(members, {
      "List members → 200": (r) => r.status === 200,
      "At least 2 members": (r) => r.json().items.length >= 2,
    });
  });

  // ── 5. Messages ─────────────────────────────────────────────────────────
  let messageId, replyId;
  group("5. Messages", () => {
    const msg = http.post(
      `${GATEWAY}/api/channels/${channelId}/messages`,
      JSON.stringify({
        content: "Hello team! This is the first message in #smoke-test.",
      }),
      jsonAuthHeaders(aliceToken),
    );
    check(msg, {
      "Alice sends message → 201": (r) => r.status === 201,
      "Message ID present": (r) =>
        typeof r.json().id === "string" && r.json().id.startsWith("msg_"),
    });
    messageId = msg.json().id;

    const reply = http.post(
      `${GATEWAY}/api/channels/${channelId}/messages`,
      JSON.stringify({
        content: "Hey Alice! Great to be here.",
        threadId: messageId,
      }),
      jsonAuthHeaders(bobToken),
    );
    check(reply, {
      "Bob sends reply → 201": (r) => r.status === 201,
      "Reply threadId matches parent": (r) => r.json().threadId === messageId,
    });
    replyId = reply.json().id;

    const thread = http.get(
      `${GATEWAY}/api/channels/${channelId}/messages/${messageId}/replies`,
      authHeaders(aliceToken),
    );
    check(thread, {
      "Get thread replies → 200": (r) => r.status === 200,
      "Has reply items": (r) => Array.isArray(r.json().items),
    });

    const listMsgs = http.get(
      `${GATEWAY}/api/channels/${channelId}/messages`,
      authHeaders(aliceToken),
    );
    check(listMsgs, {
      "List channel messages → 200": (r) => r.status === 200,
    });

    const edit = http.patch(
      `${GATEWAY}/api/channels/${channelId}/messages/${messageId}`,
      JSON.stringify({
        content: "Hello team! First message in #smoke-test. (edited)",
      }),
      jsonAuthHeaders(aliceToken),
    );
    check(edit, {
      "Edit message → 200": (r) => r.status === 200,
      "Content updated": (r) => r.json().content.includes("(edited)"),
      "editedAt is set": (r) => r.json().editedAt !== null,
    });

    const single = http.get(
      `${GATEWAY}/api/channels/${channelId}/messages/${messageId}`,
      authHeaders(aliceToken),
    );
    check(single, { "Get single message → 200": (r) => r.status === 200 });
  });

  // ── 6. Reactions ────────────────────────────────────────────────────────
  group("6. Reactions", () => {
    const add = http.put(
      `${GATEWAY}/api/channels/${channelId}/messages/${replyId}/reactions/%F0%9F%91%8D`,
      null,
      authHeaders(aliceToken),
    );
    check(add, { "Add reaction → 204": (r) => r.status === 204 });

    const get = http.get(
      `${GATEWAY}/api/channels/${channelId}/messages/${replyId}`,
      authHeaders(aliceToken),
    );
    check(get, {
      "Message has reactions": (r) => r.status === 200 && r.json().reactions !== undefined,
    });

    const rm = http.del(
      `${GATEWAY}/api/channels/${channelId}/messages/${replyId}/reactions/%F0%9F%91%8D`,
      null,
      authHeaders(aliceToken),
    );
    check(rm, { "Remove reaction → 204": (r) => r.status === 204 });
  });

  // ── 7. Pins ───────────────────────────────────────────────────────────
  group("7. Pins", () => {
    const pin = http.put(
      `${GATEWAY}/api/channels/${channelId}/messages/${messageId}/pin`,
      null,
      authHeaders(aliceToken),
    );
    check(pin, { "Pin message → 204": (r) => r.status === 204 });

    const list = http.get(
      `${GATEWAY}/api/channels/${channelId}/pins`,
      authHeaders(aliceToken),
    );
    check(list, {
      "List pins → 200": (r) => r.status === 200,
      "Pinned message in list": (r) =>
        Array.isArray(r.json().items) &&
        r.json().items.some((m) => m.id === messageId),
    });

    const unpin = http.del(
      `${GATEWAY}/api/channels/${channelId}/messages/${messageId}/pin`,
      null,
      authHeaders(aliceToken),
    );
    check(unpin, { "Unpin message → 204": (r) => r.status === 204 });

    const empty = http.get(
      `${GATEWAY}/api/channels/${channelId}/pins`,
      authHeaders(aliceToken),
    );
    check(empty, {
      "Pins list empty after unpin": (r) =>
        r.status === 200 && r.json().items.length === 0,
    });
  });

  // ── 8. File Attachments ───────────────────────────────────────────────
  let attachmentId;
  group("8. Attachments", () => {
    // Text file upload
    const txtUpload = http.post(
      `${GATEWAY}/api/channels/${channelId}/messages`,
      {
        content: "Check out this file!",
        files: http.file(HELLO_TXT, "hello.txt", "text/plain"),
      },
      { headers: { Authorization: `Bearer ${aliceToken}`, Accept: "application/json" } },
    );
    check(txtUpload, {
      "Upload text file → 201": (r) => r.status === 201,
      "Text filename matches": (r) => r.json().attachments[0].fileName === "hello.txt",
    });
    attachmentId = txtUpload.json().attachments[0].id;

    const txtDownload = http.get(
      `${GATEWAY}/api/attachments/${attachmentId}`,
      { headers: { Authorization: `Bearer ${aliceToken}` } },
    );
    check(txtDownload, {
      "Download text file → 200": (r) => r.status === 200,
      "Text file integrity OK": (r) => r.body === HELLO_TXT,
    });

    // Auth guard
    const noAuth = http.get(`${GATEWAY}/api/attachments/${attachmentId}`);
    check(noAuth, { "Download without auth → 401": (r) => r.status === 401 });

    // Attachment in message metadata
    const msgMeta = http.get(
      `${GATEWAY}/api/channels/${channelId}/messages/${txtUpload.json().id}`,
      authHeaders(aliceToken),
    );
    check(msgMeta, {
      "Attachment in message metadata": (r) =>
        r.status === 200 && r.body.includes(attachmentId),
    });

    // Small PNG upload + integrity (SHA-256)
    const smallUpload = http.post(
      `${GATEWAY}/api/channels/${channelId}/messages`,
      {
        content: "Small avatar",
        files: http.file(SMALL_PNG, "Digital_punk_pirate_avatar_small.png", "image/png"),
      },
      { headers: { Authorization: `Bearer ${aliceToken}`, Accept: "application/json" } },
    );
    check(smallUpload, {
      "Upload small PNG → 201": (r) => r.status === 201,
      "Small PNG filename matches": (r) =>
        r.json().attachments[0].fileName === "Digital_punk_pirate_avatar_small.png",
    });
    const smallAttId = smallUpload.json().attachments[0].id;

    const smallDownload = http.get(
      `${GATEWAY}/api/attachments/${smallAttId}`,
      {
        headers: { Authorization: `Bearer ${aliceToken}` },
        responseType: "binary",
      },
    );
    const smallOrigHash = sha256(SMALL_PNG, "hex");
    const smallDlHash = sha256(smallDownload.body, "hex");
    check(null, {
      "Small PNG integrity OK (SHA-256)": () => smallDlHash === smallOrigHash,
    });

    // Large PNG upload + integrity (SHA-256)
    const largeUpload = http.post(
      `${GATEWAY}/api/channels/${channelId}/messages`,
      {
        content: "Large avatar",
        files: http.file(LARGE_PNG, "Digital_punk_pirate_avatar.png", "image/png"),
      },
      { headers: { Authorization: `Bearer ${aliceToken}`, Accept: "application/json" } },
    );
    check(largeUpload, {
      "Upload large PNG (2.3 MB) → 201": (r) => r.status === 201,
    });
    const largeAttId = largeUpload.json().attachments[0].id;

    const largeDownload = http.get(
      `${GATEWAY}/api/attachments/${largeAttId}`,
      {
        headers: { Authorization: `Bearer ${aliceToken}` },
        responseType: "binary",
      },
    );
    const largeOrigHash = sha256(LARGE_PNG, "hex");
    const largeDlHash = sha256(largeDownload.body, "hex");
    check(null, {
      "Large PNG integrity OK (SHA-256)": () => largeDlHash === largeOrigHash,
    });
  });

  // ── 9. Search ────────────────────────────────────────────────────────
  group("9. Search", () => {
    const search = http.get(
      `${GATEWAY}/api/search/messages?q=first%20message`,
      authHeaders(aliceToken),
    );
    check(search, {
      "Search messages → 200": (r) => r.status === 200,
      "Has search results": (r) => Array.isArray(r.json().items),
    });

    const scoped = http.get(
      `${GATEWAY}/api/search/messages?q=file&channelId=${channelId}`,
      authHeaders(aliceToken),
    );
    check(scoped, {
      "Search scoped to channel → 200": (r) => r.status === 200,
    });

    const empty = http.get(
      `${GATEWAY}/api/search/messages?q=`,
      authHeaders(aliceToken),
    );
    check(empty, { "Empty search → 400": (r) => r.status === 400 });
  });

  // ── 10. Pagination ─────────────────────────────────────────────────────
  group("10. Pagination", () => {
    // Add messages for pagination
    http.post(
      `${GATEWAY}/api/channels/${channelId}/messages`,
      JSON.stringify({ content: "Pagination msg 1" }),
      jsonAuthHeaders(aliceToken),
    );
    http.post(
      `${GATEWAY}/api/channels/${channelId}/messages`,
      JSON.stringify({ content: "Pagination msg 2" }),
      jsonAuthHeaders(bobToken),
    );

    const page1 = http.get(
      `${GATEWAY}/api/channels/${channelId}/messages?limit=2`,
      authHeaders(aliceToken),
    );
    check(page1, {
      "Paginated fetch (limit=2) → 200": (r) => r.status === 200,
      "Returns ≤2 items": (r) => r.json().items.length <= 2,
      "Has cursor for next page": (r) =>
        typeof r.json().cursor === "string" && r.json().cursor.length > 0,
    });

    const cursor = page1.json().cursor;
    const page2 = http.get(
      `${GATEWAY}/api/channels/${channelId}/messages?limit=2&cursor=${cursor}`,
      authHeaders(aliceToken),
    );
    check(page2, { "Fetch page 2 → 200": (r) => r.status === 200 });
  });

  // ── 11. Mark Read ──────────────────────────────────────────────────────
  group("11. Mark Read", () => {
    const mark = http.patch(
      `${GATEWAY}/api/channels/${channelId}/members/me/last-read`,
      null,
      authHeaders(bobToken),
    );
    check(mark, { "Mark channel as read → 204": (r) => r.status === 204 });
  });

  // ── 12. User Profile ───────────────────────────────────────────────────
  let bobId;
  group("12. User Profile", () => {
    const me = http.get(`${GATEWAY}/api/users/me`, authHeaders(aliceToken));
    check(me, {
      "Get /users/me → 200": (r) => r.status === 200,
      "Has displayName": (r) => typeof r.json().displayName === "string",
      "Has role": (r) => r.json().role !== undefined,
    });
    const bobMe = http.get(`${GATEWAY}/api/users/me`, authHeaders(bobToken));
    bobId = bobMe.json().id;

    const users = http.get(`${GATEWAY}/api/users`, authHeaders(aliceToken));
    check(users, {
      "List users → 200": (r) => r.status === 200,
      "Users has items": (r) => Array.isArray(r.json().items),
      "At least 2 users": (r) => r.json().items.length >= 2,
    });

    const getUser = http.get(
      `${GATEWAY}/api/users/${bobId}`,
      authHeaders(aliceToken),
    );
    check(getUser, {
      "Get user by ID → 200": (r) => r.status === 200,
      "User ID matches": (r) => r.json().id === bobId,
    });
  });

  // ── 13. Direct Messages ───────────────────────────────────────────────
  group("13. Direct Messages", () => {
    const dm = http.post(
      `${GATEWAY}/api/dms`,
      JSON.stringify({ userId: bobId }),
      jsonAuthHeaders(aliceToken),
    );
    check(dm, {
      "Create DM → 200": (r) => r.status === 200,
      "DM kind is dm": (r) => r.json().kind === "dm",
    });
    const dmId = dm.json().id;

    // Idempotent: creating again returns the same DM
    const dm2 = http.post(
      `${GATEWAY}/api/dms`,
      JSON.stringify({ userId: bobId }),
      jsonAuthHeaders(aliceToken),
    );
    check(dm2, {
      "DM is idempotent": (r) => r.json().id === dmId,
    });

    // Send a message in the DM
    const msg = http.post(
      `${GATEWAY}/api/channels/${dmId}/messages`,
      JSON.stringify({ content: "Hey Bob, private message!" }),
      jsonAuthHeaders(aliceToken),
    );
    check(msg, {
      "Send DM message → 201": (r) => r.status === 201,
    });
  });

  // ── 14. Notify Preferences ────────────────────────────────────────────
  group("14. Notify Preferences", () => {
    const setMentions = http.patch(
      `${GATEWAY}/api/channels/${channelId}/members/me/notify`,
      JSON.stringify({ notify: "mentions" }),
      jsonAuthHeaders(bobToken),
    );
    check(setMentions, { "Set notify to mentions → 204": (r) => r.status === 204 });

    const setAll = http.patch(
      `${GATEWAY}/api/channels/${channelId}/members/me/notify`,
      JSON.stringify({ notify: "all" }),
      jsonAuthHeaders(bobToken),
    );
    check(setAll, { "Set notify back to all → 204": (r) => r.status === 204 });
  });

  // ── 15. Archive / Unarchive ───────────────────────────────────────────
  group("15. Archive / Unarchive", () => {
    const archive = http.post(
      `${GATEWAY}/api/channels/${channelId}/archive`,
      null,
      authHeaders(aliceToken),
    );
    check(archive, {
      "Archive channel → 200": (r) => r.status === 200,
      "isArchived is true": (r) => r.json().isArchived === true,
    });

    // Sending to archived channel should fail
    const blocked = http.post(
      `${GATEWAY}/api/channels/${channelId}/messages`,
      JSON.stringify({ content: "Should be blocked" }),
      jsonAuthHeaders(aliceToken),
    );
    check(blocked, {
      "Send to archived channel → 403": (r) => r.status === 403,
    });

    const unarchive = http.post(
      `${GATEWAY}/api/channels/${channelId}/unarchive`,
      null,
      authHeaders(aliceToken),
    );
    check(unarchive, {
      "Unarchive channel → 200": (r) => r.status === 200,
      "isArchived is false": (r) => r.json().isArchived === false,
    });
  });

  // ── 16. Admin Panel ───────────────────────────────────────────────────
  group("16. Admin Panel", () => {
    // Non-admin should be rejected
    const forbidden = http.get(`${GATEWAY}/api/admin/users`, authHeaders(bobToken));
    check(forbidden, { "Non-admin → admin/users 403": (r) => r.status === 403 });

    const users = http.get(`${GATEWAY}/api/admin/users`, authHeaders(aliceToken));
    check(users, {
      "Admin list users → 200": (r) => r.status === 200,
      "Admin users has items": (r) => Array.isArray(r.json().items),
    });

    const channels = http.get(`${GATEWAY}/api/admin/channels`, authHeaders(aliceToken));
    check(channels, {
      "Admin list channels → 200": (r) => r.status === 200,
      "Admin channels has items": (r) => Array.isArray(r.json().items),
    });

    // Update a user role (set bob to moderator, then back to member)
    const promote = http.patch(
      `${GATEWAY}/api/admin/users/${bobId}`,
      JSON.stringify({ role: "moderator" }),
      jsonAuthHeaders(aliceToken),
    );
    check(promote, {
      "Admin promote bob → 200": (r) => r.status === 200,
      "Bob is now moderator": (r) => r.json().role === "moderator",
    });

    const demote = http.patch(
      `${GATEWAY}/api/admin/users/${bobId}`,
      JSON.stringify({ role: "member" }),
      jsonAuthHeaders(aliceToken),
    );
    check(demote, {
      "Admin demote bob → 200": (r) => r.status === 200,
      "Bob is back to member": (r) => r.json().role === "member",
    });

    // Check audit log after role changes so it's guaranteed non-empty
    const audit = http.get(`${GATEWAY}/api/admin/audit-log`, authHeaders(aliceToken));
    check(audit, {
      "Admin audit log → 200": (r) => r.status === 200,
      "Audit log has items": (r) => Array.isArray(r.json().items),
      "Audit log not empty": (r) => r.json().items.length > 0,
    });
  });

  // ── 17. Error Cases ────────────────────────────────────────────────────
  group("17. Error Cases", () => {
    const fake = "ch_00000000-0000-7000-0000-000000000000";

    const notFound = http.get(
      `${GATEWAY}/api/channels/${fake}`,
      authHeaders(aliceToken),
    );
    check(notFound, {
      "Non-existent channel → 404": (r) => r.status === 404,
      "RFC 9457 error type": (r) => r.json().type !== undefined,
    });

    const postFake = http.post(
      `${GATEWAY}/api/channels/${fake}/messages`,
      JSON.stringify({ content: "Should fail" }),
      jsonAuthHeaders(aliceToken),
    );
    check(postFake, {
      "Send to non-existent channel → 403|404": (r) =>
        r.status === 403 || r.status === 404,
    });

    const emptyMsg = http.post(
      `${GATEWAY}/api/channels/${channelId}/messages`,
      JSON.stringify({ content: "" }),
      jsonAuthHeaders(aliceToken),
    );
    check(emptyMsg, { "Empty message → 400": (r) => r.status === 400 });

    const fakeAtt = http.get(
      `${GATEWAY}/api/attachments/att_00000000-0000-7000-0000-000000000000`,
      authHeaders(aliceToken),
    );
    check(fakeAtt, {
      "Non-existent attachment → 404": (r) => r.status === 404,
    });
  });

  // ── 18. Webhooks ───────────────────────────────────────────────────────
  let webhookId, webhookToken;
  group("18. Webhooks", () => {
    // Create incoming webhook (alice is admin, also has IntegrationUser access)
    const createIncoming = http.post(
      `${GATEWAY}/api/channels/${channelId}/webhooks`,
      JSON.stringify({ kind: "incoming", name: "Smoke Incoming" }),
      jsonAuthHeaders(aliceToken),
    );
    check(createIncoming, {
      "Create incoming webhook → 201": (r) => r.status === 201,
      "Has token": (r) => typeof r.json().token === "string" && r.json().token.length > 0,
      "Kind is incoming": (r) => r.json().kind === "incoming",
    });
    webhookId = createIncoming.json().id;
    webhookToken = createIncoming.json().token;

    // Create outgoing webhook (requires url)
    const createOutgoing = http.post(
      `${GATEWAY}/api/channels/${channelId}/webhooks`,
      JSON.stringify({
        kind: "outgoing",
        name: "Smoke Outgoing",
        url: "https://example.com/hook",
      }),
      jsonAuthHeaders(aliceToken),
    );
    check(createOutgoing, {
      "Create outgoing webhook → 201": (r) => r.status === 201,
      "Outgoing has url": (r) => r.json().url === "https://example.com/hook",
    });
    const outgoingId = createOutgoing.json().id;

    // Outgoing without url → 400
    const noUrl = http.post(
      `${GATEWAY}/api/channels/${channelId}/webhooks`,
      JSON.stringify({ kind: "outgoing", name: "No URL" }),
      jsonAuthHeaders(aliceToken),
    );
    check(noUrl, { "Outgoing without url → 400": (r) => r.status === 400 });

    // List webhooks
    const list = http.get(
      `${GATEWAY}/api/channels/${channelId}/webhooks`,
      authHeaders(aliceToken),
    );
    check(list, {
      "List webhooks → 200": (r) => r.status === 200,
      "Has 2 webhooks": (r) => r.json().items.length === 2,
    });

    // Get single webhook
    const get = http.get(
      `${GATEWAY}/api/channels/${channelId}/webhooks/${webhookId}`,
      authHeaders(aliceToken),
    );
    check(get, {
      "Get webhook → 200": (r) => r.status === 200,
      "Name matches": (r) => r.json().name === "Smoke Incoming",
    });

    // Update webhook
    const update = http.patch(
      `${GATEWAY}/api/channels/${channelId}/webhooks/${webhookId}`,
      JSON.stringify({ name: "Renamed Incoming" }),
      jsonAuthHeaders(aliceToken),
    );
    check(update, {
      "Update webhook → 200": (r) => r.status === 200,
      "Name updated": (r) => r.json().name === "Renamed Incoming",
    });

    // Trigger incoming webhook (bypasses OIDC, uses webhook token)
    const trigger = http.post(
      `${GATEWAY}/api/webhooks/${webhookId}/trigger`,
      JSON.stringify({ content: "Hello from k6 smoke test!" }),
      {
        headers: {
          Authorization: `Bearer ${webhookToken}`,
          "Content-Type": "application/json",
        },
      },
    );
    check(trigger, {
      "Trigger incoming webhook → 201": (r) => r.status === 201,
      "Message has content": (r) => r.json().content === "Hello from k6 smoke test!",
      "Message has channelId": (r) => r.json().channelId === channelId,
    });

    // Trigger with bad token → 401
    const badTrigger = http.post(
      `${GATEWAY}/api/webhooks/${webhookId}/trigger`,
      JSON.stringify({ content: "bad" }),
      {
        headers: {
          Authorization: "Bearer wrong_token",
          "Content-Type": "application/json",
        },
      },
    );
    check(badTrigger, {
      "Trigger with bad token → 401": (r) => r.status === 401,
    });

    // Trigger outgoing webhook → 400 (can't trigger outgoing)
    const triggerOutgoing = http.post(
      `${GATEWAY}/api/webhooks/${outgoingId}/trigger`,
      JSON.stringify({ content: "nope" }),
      {
        headers: {
          Authorization: `Bearer ${createOutgoing.json().token}`,
          "Content-Type": "application/json",
        },
      },
    );
    check(triggerOutgoing, {
      "Trigger outgoing webhook → 400": (r) => r.status === 400,
    });

    // Bob (member) cannot create webhooks → 403
    const bobCreate = http.post(
      `${GATEWAY}/api/channels/${channelId}/webhooks`,
      JSON.stringify({ kind: "incoming", name: "Unauthorized" }),
      jsonAuthHeaders(bobToken),
    );
    check(bobCreate, {
      "Member cannot create webhook → 403": (r) => r.status === 403,
    });

    // Admin list all webhooks
    const listAll = http.get(
      `${GATEWAY}/api/admin/webhooks`,
      authHeaders(aliceToken),
    );
    check(listAll, {
      "Admin list all webhooks → 200": (r) => r.status === 200,
      "Has items": (r) => Array.isArray(r.json().items),
    });

    // Delete outgoing webhook
    const del = http.del(
      `${GATEWAY}/api/channels/${channelId}/webhooks/${outgoingId}`,
      null,
      authHeaders(aliceToken),
    );
    check(del, { "Delete webhook → 204": (r) => r.status === 204 });
  });

  // ── 19. Bots ──────────────────────────────────────────────────────────
  group("19. Bots", () => {
    const botUsername = `smoke-bot-${Date.now()}`;

    // Create bot (admin)
    const create = http.post(
      `${GATEWAY}/api/admin/bots`,
      JSON.stringify({ username: botUsername, displayName: "Smoke Bot" }),
      jsonAuthHeaders(aliceToken),
    );
    check(create, {
      "Create bot → 201": (r) => r.status === 201,
      "Bot isBot=true": (r) => r.json().isBot === true,
      "Bot username matches": (r) => r.json().username === botUsername,
    });
    const botId = create.json().id;

    // List bots
    const list = http.get(
      `${GATEWAY}/api/admin/bots`,
      authHeaders(aliceToken),
    );
    check(list, {
      "List bots → 200": (r) => r.status === 200,
      "Contains created bot": (r) =>
        r.json().items.some((b) => b.id === botId),
    });

    // Update bot
    const update = http.patch(
      `${GATEWAY}/api/admin/bots/${botId}`,
      JSON.stringify({ displayName: "Renamed Bot" }),
      jsonAuthHeaders(aliceToken),
    );
    check(update, {
      "Update bot → 200": (r) => r.status === 200,
      "Display name updated": (r) => r.json().displayName === "Renamed Bot",
    });

    // Bob (member) cannot create bots → 403
    const bobCreate = http.post(
      `${GATEWAY}/api/admin/bots`,
      JSON.stringify({ username: "bob-bot", displayName: "Nope" }),
      jsonAuthHeaders(bobToken),
    );
    check(bobCreate, {
      "Member cannot create bot → 403": (r) => r.status === 403,
    });

    // Deactivate bot
    const deactivate = http.del(
      `${GATEWAY}/api/admin/bots/${botId}`,
      null,
      authHeaders(aliceToken),
    );
    check(deactivate, { "Deactivate bot → 204": (r) => r.status === 204 });

    // Duplicate username → 409
    const dup1 = http.post(
      `${GATEWAY}/api/admin/bots`,
      JSON.stringify({ username: "dup-bot", displayName: "Dup 1" }),
      jsonAuthHeaders(aliceToken),
    );
    check(dup1, { "Create bot for dup test → 201": (r) => r.status === 201 });
    const dup2 = http.post(
      `${GATEWAY}/api/admin/bots`,
      JSON.stringify({ username: "dup-bot", displayName: "Dup 2" }),
      jsonAuthHeaders(aliceToken),
    );
    check(dup2, { "Duplicate bot username → 409": (r) => r.status === 409 });
  });

  // ── 20. Cleanup ────────────────────────────────────────────────────────
  group("20. Cleanup", () => {
    const del = http.del(
      `${GATEWAY}/api/channels/${channelId}/messages/${replyId}`,
      null,
      authHeaders(bobToken),
    );
    check(del, { "Delete reply → 204": (r) => r.status === 204 });

    const soft = http.get(
      `${GATEWAY}/api/channels/${channelId}/messages/${replyId}`,
      authHeaders(aliceToken),
    );
    check(soft, {
      "Soft-deleted message still 200": (r) => r.status === 200,
      "deletedAt is set": (r) => r.json().deletedAt !== null,
    });

    const leave = http.del(
      `${GATEWAY}/api/channels/${channelId}/members/me`,
      null,
      authHeaders(bobToken),
    );
    check(leave, { "Bob leaves channel → 204": (r) => r.status === 204 });

    const delChannel = http.del(
      `${GATEWAY}/api/admin/channels/${channelId}`,
      null,
      authHeaders(aliceToken),
    );
    check(delChannel, { "Admin deletes channel → 204": (r) => r.status === 204 });
  });
}

// ── Rate-limit scenario ──────────────────────────────────────────────────────
// Fires requests beyond the gateway quota (300 req / 60 s) and verifies that
// the gateway returns 429 with the expected headers.

export function rateLimit() {
  const token = getToken("bob");
  const endpoint = `${GATEWAY}/api/channels`;

  let got429 = false;
  let hasRetryAfter = false;

  // Exceed the 300 req / 60 s quota. Stop as soon as we see 429.
  for (let i = 0; i < 320; i++) {
    const res = http.get(endpoint, authHeaders(token));
    if (res.status === 429) {
      got429 = true;
      hasRetryAfter = res.headers["Retry-After"] !== undefined;
      break;
    }
  }

  check(null, {
    "Rate limit triggers 429": () => got429,
    "429 includes Retry-After header": () => hasRetryAfter,
  });
}
