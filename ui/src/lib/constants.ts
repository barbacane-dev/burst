// ── WebSocket ────────────────────────────────────────────────────────────────
export const WS_HEARTBEAT_INTERVAL_MS = 20_000;

// ── File uploads ─────────────────────────────────────────────────────────────
export const MAX_FILE_SIZE_BYTES = 10 * 1024 * 1024; // 10 MB (matches backend)
export const MAX_FILES_PER_MESSAGE = 10;

// ── Storage keys ─────────────────────────────────────────────────────────────
export const STORAGE_KEY_THEME = "burst-theme";
export const STORAGE_KEY_ACCESS_TOKEN = "burst_access_token";
