/**
 * Burst API — S3 storage smoke test
 *
 * Tests the full path: Burst (GatewayStorage) → Barbacane S3 dispatcher → RustFS
 *
 * Prerequisites:
 *   docker compose -f docker-compose.dev.yml up  # PostgreSQL + mock OIDC + RustFS
 *   make seed
 *   # Set S3 env vars before starting gateway:
 *   export BURST_S3_REGION=us-east-1
 *   export BURST_S3_BUCKET=burst-files
 *   export BURST_S3_ACCESS_KEY_ID=burst
 *   export BURST_S3_SECRET_ACCESS_KEY=burstpass
 *   export BURST_S3_ENDPOINT=http://localhost:9000
 *   make gateway-compile && make gateway
 *   # Start Burst with gateway storage:
 *   BURST_STORAGE_BACKEND=gateway BURST_STORAGE_GATEWAY_URL=http://localhost:8080 make server
 *
 * Usage:
 *   k6 run tests/http/smoke-s3.js
 *
 * Note: The RustFS bucket must be created before running. Use the RustFS console
 * at http://localhost:9001 or: curl -X PUT http://localhost:9000/burst-files
 */

import http from "k6/http";
import { check, group } from "k6";
import { sha256 } from "k6/crypto";

const GATEWAY = __ENV.GATEWAY_URL || "http://localhost:8080";
const MOCK_OAUTH = __ENV.MOCK_OAUTH_URL || "http://localhost:9099";

const SMALL_PNG = open("./fixtures/Digital_punk_pirate_avatar_small.png", "b");

export const options = {
  scenarios: {
    s3smoke: {
      executor: "per-vu-iterations",
      vus: 1,
      iterations: 1,
      maxDuration: "2m",
      exec: "s3smoke",
    },
  },
  thresholds: {
    checks: [{ threshold: "rate==1.0" }],
  },
};

function getToken(username) {
  const res = http.post(
    `${MOCK_OAUTH}/burst/token`,
    `grant_type=password&username=${username}&password=pass&client_id=burst`,
    { headers: { "Content-Type": "application/x-www-form-urlencoded" } }
  );
  return JSON.parse(res.body).access_token;
}

export function s3smoke() {
  const token = getToken("alice");
  const auth = { headers: { Authorization: `Bearer ${token}` } };

  // ── 1. Create channel and send message with file attachment ──────────

  group("S3: Upload file via message", () => {
    // Create a channel for testing
    const ch = http.post(
      `${GATEWAY}/api/channels`,
      JSON.stringify({ name: `s3-test-${Date.now()}` }),
      { headers: { ...auth.headers, "Content-Type": "application/json" } }
    );
    check(ch, { "Create channel → 201": (r) => r.status === 201 });
    const channelId = JSON.parse(ch.body).id;

    // Send a message with a PNG attachment (multipart)
    const pngHash = sha256(SMALL_PNG, "hex");
    const msg = http.post(
      `${GATEWAY}/api/channels/${channelId}/messages`,
      {
        content: http.file("s3 storage test message", "content", "text/plain"),
        files: http.file(SMALL_PNG, "test-s3.png", "image/png"),
      },
      auth
    );
    check(msg, {
      "Send message with attachment → 201": (r) => r.status === 201,
      "Attachment present": (r) => JSON.parse(r.body).attachments.length > 0,
    });

    const attachment = JSON.parse(msg.body).attachments[0];

    // ── 2. Download the file and verify integrity ──────────────────────

    const dl = http.get(`${GATEWAY}/api/attachments/${attachment.id}`, auth);
    check(dl, {
      "Download attachment → 200": (r) => r.status === 200,
      "Content-Type is image/png": (r) =>
        r.headers["Content-Type"].startsWith("image/png"),
      "SHA-256 matches": (r) => sha256(r.body, "hex") === pngHash,
    });

    // ── 3. Delete the message (soft delete) ───────────────────────────

    const msgId = JSON.parse(msg.body).id;
    const del = http.del(
      `${GATEWAY}/api/channels/${channelId}/messages/${msgId}`,
      null,
      auth
    );
    check(del, { "Delete message → 204": (r) => r.status === 204 });
  });
}
