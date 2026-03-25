/**
 * Playwright global setup: cleans up leftover e2e test channels before the
 * test suite runs. This prevents rate-limit errors and test pollution from
 * accumulated runs.
 *
 * Authenticates as alice (admin) via the mock OAuth server, then uses the
 * admin API to list and delete channels whose names match e2e prefixes.
 */

const BASE_URL = "http://localhost:5173";

/** Channel name prefixes created by e2e tests. */
const E2E_PREFIXES = ["test-", "pin-", "mention-", "multi-", "realtime-"];

function isTestChannel(name: string): boolean {
  return E2E_PREFIXES.some((prefix) => name.startsWith(prefix));
}

async function getAdminToken(): Promise<string> {
  const params = new URLSearchParams({
    grant_type: "password",
    username: "alice",
    password: "password",
    client_id: "burst",
    scope: "openid",
  });

  const res = await fetch(`${BASE_URL}/oauth/burst/token`, {
    method: "POST",
    headers: { "Content-Type": "application/x-www-form-urlencoded" },
    body: params,
  });

  if (!res.ok) {
    throw new Error(`Failed to get admin token: ${res.status}`);
  }

  const data = await res.json();
  return data.access_token;
}

interface Channel {
  id: string;
  name?: string;
}

interface AdminChannelList {
  items: Channel[];
  cursor?: string;
}

async function deleteTestChannels(token: string): Promise<void> {
  const headers = {
    Authorization: `Bearer ${token}`,
    "Content-Type": "application/json",
  };

  // Paginate through all channels.
  let cursor: string | undefined;
  let deleted = 0;

  do {
    const url = new URL(`${BASE_URL}/api/admin/channels`);
    url.searchParams.set("limit", "50");
    if (cursor) {
      url.searchParams.set("cursor", cursor);
    }

    const res = await fetch(url, { headers });
    if (!res.ok) {
      throw new Error(`Failed to list channels: ${res.status}`);
    }

    const data: AdminChannelList = await res.json();

    for (const channel of data.items) {
      if (channel.name && isTestChannel(channel.name)) {
        const delRes = await fetch(
          `${BASE_URL}/api/admin/channels/${channel.id}`,
          { method: "DELETE", headers },
        );
        if (delRes.ok || delRes.status === 204) {
          deleted++;
        }
      }
    }

    cursor = data.cursor;
  } while (cursor);

  if (deleted > 0) {
    console.log(`  Cleaned up ${deleted} leftover e2e test channel(s).`);
  }
}

export default async function globalSetup(): Promise<void> {
  const token = await getAdminToken();
  await deleteTestChannels(token);
}
