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
  // Smoke test: single user, single iteration, no ramping
  vus: 1,
  iterations: 1,
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

export default function () {
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

    const burst = http.get(`${BURST}/channels`, {
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

  // ── 2. Gateway Auth Validation ───────────────────────────────────────────
  group("2. Gateway Auth", () => {
    const noAuth = http.get(`${GATEWAY}/channels`, {
      headers: { Accept: "application/json" },
    });
    check(noAuth, { "No auth → 401": (r) => r.status === 401 });

    const badAuth = http.get(`${GATEWAY}/channels`, {
      headers: { Accept: "application/json", Authorization: "Bearer bad-jwt" },
    });
    check(badAuth, { "Bad token → 401": (r) => r.status === 401 });

    const goodAuth = http.get(`${GATEWAY}/channels`, authHeaders(aliceToken));
    check(goodAuth, {
      "Valid token passes gateway → 200": (r) => r.status === 200,
    });
  });

  // ── 3. Channels CRUD ────────────────────────────────────────────────────
  let channelId;
  group("3. Channels CRUD", () => {
    const list = http.get(`${GATEWAY}/channels`, authHeaders(aliceToken));
    check(list, {
      "List channels → 200": (r) => r.status === 200,
      "Has items array": (r) => Array.isArray(r.json().items),
    });

    const create = http.post(
      `${GATEWAY}/channels`,
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
      `${GATEWAY}/channels/${channelId}`,
      authHeaders(aliceToken),
    );
    check(get, { "Get channel by ID → 200": (r) => r.status === 200 });

    const dup = http.post(
      `${GATEWAY}/channels`,
      JSON.stringify({ name: "Dup", slug: "smoke-test", kind: "public" }),
      jsonAuthHeaders(aliceToken),
    );
    check(dup, { "Duplicate slug → 409": (r) => r.status === 409 });

    const patch = http.patch(
      `${GATEWAY}/channels/${channelId}`,
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
      `${GATEWAY}/channels/${channelId}/members`,
      null,
      authHeaders(bobToken),
    );
    check(join, { "Bob joins channel → 204": (r) => r.status === 204 });

    const members = http.get(
      `${GATEWAY}/channels/${channelId}/members`,
      authHeaders(aliceToken),
    );
    check(members, {
      "List members → 200": (r) => r.status === 200,
      "At least 2 members": (r) => r.json().length >= 2,
    });
  });

  // ── 5. Messages ─────────────────────────────────────────────────────────
  let messageId, replyId;
  group("5. Messages", () => {
    const msg = http.post(
      `${GATEWAY}/channels/${channelId}/messages`,
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
      `${GATEWAY}/channels/${channelId}/messages`,
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
      `${GATEWAY}/channels/${channelId}/messages/${messageId}/replies`,
      authHeaders(aliceToken),
    );
    check(thread, {
      "Get thread replies → 200": (r) => r.status === 200,
      "Has reply items": (r) => Array.isArray(r.json().items),
    });

    const listMsgs = http.get(
      `${GATEWAY}/channels/${channelId}/messages`,
      authHeaders(aliceToken),
    );
    check(listMsgs, {
      "List channel messages → 200": (r) => r.status === 200,
    });

    const edit = http.patch(
      `${GATEWAY}/channels/${channelId}/messages/${messageId}`,
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
      `${GATEWAY}/channels/${channelId}/messages/${messageId}`,
      authHeaders(aliceToken),
    );
    check(single, { "Get single message → 200": (r) => r.status === 200 });
  });

  // ── 6. Reactions ────────────────────────────────────────────────────────
  group("6. Reactions", () => {
    const add = http.put(
      `${GATEWAY}/channels/${channelId}/messages/${replyId}/reactions/%F0%9F%91%8D`,
      null,
      authHeaders(aliceToken),
    );
    check(add, { "Add reaction → 204": (r) => r.status === 204 });

    const get = http.get(
      `${GATEWAY}/channels/${channelId}/messages/${replyId}`,
      authHeaders(aliceToken),
    );
    check(get, {
      "Message has reactions": (r) => r.status === 200 && r.json().reactions !== undefined,
    });

    const rm = http.del(
      `${GATEWAY}/channels/${channelId}/messages/${replyId}/reactions/%F0%9F%91%8D`,
      null,
      authHeaders(aliceToken),
    );
    check(rm, { "Remove reaction → 204": (r) => r.status === 204 });
  });

  // ── 7. File Attachments ─────────────────────────────────────────────────
  let attachmentId;
  group("7. Attachments", () => {
    // Text file upload
    const txtUpload = http.post(
      `${GATEWAY}/channels/${channelId}/messages`,
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
      `${GATEWAY}/attachments/${attachmentId}`,
      { headers: { Authorization: `Bearer ${aliceToken}` } },
    );
    check(txtDownload, {
      "Download text file → 200": (r) => r.status === 200,
      "Text file integrity OK": (r) => r.body === HELLO_TXT,
    });

    // Auth guard
    const noAuth = http.get(`${GATEWAY}/attachments/${attachmentId}`);
    check(noAuth, { "Download without auth → 401": (r) => r.status === 401 });

    // Attachment in message metadata
    const msgMeta = http.get(
      `${GATEWAY}/channels/${channelId}/messages/${txtUpload.json().id}`,
      authHeaders(aliceToken),
    );
    check(msgMeta, {
      "Attachment in message metadata": (r) =>
        r.status === 200 && r.body.includes(attachmentId),
    });

    // Small PNG upload + integrity (SHA-256)
    const smallUpload = http.post(
      `${GATEWAY}/channels/${channelId}/messages`,
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
      `${GATEWAY}/attachments/${smallAttId}`,
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
      `${GATEWAY}/channels/${channelId}/messages`,
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
      `${GATEWAY}/attachments/${largeAttId}`,
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

  // ── 8. Search ───────────────────────────────────────────────────────────
  group("8. Search", () => {
    const search = http.get(
      `${GATEWAY}/search/messages?q=first%20message`,
      authHeaders(aliceToken),
    );
    check(search, {
      "Search messages → 200": (r) => r.status === 200,
      "Has search results": (r) => Array.isArray(r.json().items),
    });

    const scoped = http.get(
      `${GATEWAY}/search/messages?q=file&channelId=${channelId}`,
      authHeaders(aliceToken),
    );
    check(scoped, {
      "Search scoped to channel → 200": (r) => r.status === 200,
    });

    const empty = http.get(
      `${GATEWAY}/search/messages?q=`,
      authHeaders(aliceToken),
    );
    check(empty, { "Empty search → 400": (r) => r.status === 400 });
  });

  // ── 9. Pagination ──────────────────────────────────────────────────────
  group("9. Pagination", () => {
    // Add messages for pagination
    http.post(
      `${GATEWAY}/channels/${channelId}/messages`,
      JSON.stringify({ content: "Pagination msg 1" }),
      jsonAuthHeaders(aliceToken),
    );
    http.post(
      `${GATEWAY}/channels/${channelId}/messages`,
      JSON.stringify({ content: "Pagination msg 2" }),
      jsonAuthHeaders(bobToken),
    );

    const page1 = http.get(
      `${GATEWAY}/channels/${channelId}/messages?limit=2`,
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
      `${GATEWAY}/channels/${channelId}/messages?limit=2&cursor=${cursor}`,
      authHeaders(aliceToken),
    );
    check(page2, { "Fetch page 2 → 200": (r) => r.status === 200 });
  });

  // ── 10. Mark Read ──────────────────────────────────────────────────────
  group("10. Mark Read", () => {
    const mark = http.patch(
      `${GATEWAY}/channels/${channelId}/members/me/last-read`,
      null,
      authHeaders(bobToken),
    );
    check(mark, { "Mark channel as read → 204": (r) => r.status === 204 });
  });

  // ── 11. Error Cases ────────────────────────────────────────────────────
  group("11. Error Cases", () => {
    const fake = "ch_00000000-0000-7000-0000-000000000000";

    const notFound = http.get(
      `${GATEWAY}/channels/${fake}`,
      authHeaders(aliceToken),
    );
    check(notFound, {
      "Non-existent channel → 404": (r) => r.status === 404,
      "RFC 9457 error type": (r) => r.json().type !== undefined,
    });

    const postFake = http.post(
      `${GATEWAY}/channels/${fake}/messages`,
      JSON.stringify({ content: "Should fail" }),
      jsonAuthHeaders(aliceToken),
    );
    check(postFake, {
      "Send to non-existent channel → 403|404": (r) =>
        r.status === 403 || r.status === 404,
    });

    const emptyMsg = http.post(
      `${GATEWAY}/channels/${channelId}/messages`,
      JSON.stringify({ content: "" }),
      jsonAuthHeaders(aliceToken),
    );
    check(emptyMsg, { "Empty message → 400": (r) => r.status === 400 });

    const fakeAtt = http.get(
      `${GATEWAY}/attachments/att_00000000-0000-7000-0000-000000000000`,
      authHeaders(aliceToken),
    );
    check(fakeAtt, {
      "Non-existent attachment → 404": (r) => r.status === 404,
    });
  });

  // ── 12. Cleanup ────────────────────────────────────────────────────────
  group("12. Cleanup", () => {
    const del = http.del(
      `${GATEWAY}/channels/${channelId}/messages/${replyId}`,
      null,
      authHeaders(bobToken),
    );
    check(del, { "Delete reply → 204": (r) => r.status === 204 });

    const soft = http.get(
      `${GATEWAY}/channels/${channelId}/messages/${replyId}`,
      authHeaders(aliceToken),
    );
    check(soft, {
      "Soft-deleted message still 200": (r) => r.status === 200,
      "deletedAt is set": (r) => r.json().deletedAt !== null,
    });

    const leave = http.del(
      `${GATEWAY}/channels/${channelId}/members/me`,
      null,
      authHeaders(bobToken),
    );
    check(leave, { "Bob leaves channel → 204": (r) => r.status === 204 });
  });
}
