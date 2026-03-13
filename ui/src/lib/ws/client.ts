import { getAccessToken } from "../api/client";

type EventHandler = (event: unknown) => void;

const RECONNECT_BASE_MS = 1_000;
const RECONNECT_MAX_MS = 30_000;

class WsClient {
  private ws: WebSocket | null = null;
  private handlers = new Map<string, Set<EventHandler>>();
  private reconnectDelay = RECONNECT_BASE_MS;
  private lastEventId: string | null = null;
  private stopped = false;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;

  connect(): void {
    if (this.ws || this.stopped) return;
    this._open();
  }

  private _open(): void {
    const protocol = location.protocol === "https:" ? "wss" : "ws";
    let url = `${protocol}://${location.host}/ws`;

    const params = new URLSearchParams();
    const token = getAccessToken();
    if (token) params.set("access_token", token);
    if (this.lastEventId) params.set("lastEventId", this.lastEventId);
    if (params.size) url += `?${params}`;

    const ws = new WebSocket(url);
    this.ws = ws;

    ws.onopen = () => {
      this.reconnectDelay = RECONNECT_BASE_MS;
    };

    ws.onmessage = (e: MessageEvent) => {
      let msg: { type: string; eventId?: string } | null = null;
      try {
        msg = JSON.parse(e.data as string);
      } catch {
        return;
      }
      if (!msg) return;
      if (msg.eventId) this.lastEventId = msg.eventId;
      this.handlers.get(msg.type)?.forEach((h) => h(msg));
    };

    ws.onclose = () => {
      this.ws = null;
      if (this.stopped) return;
      this.reconnectTimer = setTimeout(() => {
        this.reconnectTimer = null;
        this._open();
      }, this.reconnectDelay);
      this.reconnectDelay = Math.min(this.reconnectDelay * 2, RECONNECT_MAX_MS);
    };

    ws.onerror = () => ws.close();
  }

  on(type: string, handler: EventHandler): () => void {
    if (!this.handlers.has(type)) this.handlers.set(type, new Set());
    this.handlers.get(type)!.add(handler);
    return () => this.handlers.get(type)?.delete(handler);
  }

  send(data: object): void {
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(data));
    }
  }

  startHeartbeat(): () => void {
    const id = setInterval(() => this.send({ type: "heartbeat" }), 20_000);
    return () => clearInterval(id);
  }

  disconnect(): void {
    this.stopped = true;
    if (this.reconnectTimer !== null) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    this.ws?.close();
    this.ws = null;
  }

  reset(): void {
    this.stopped = false;
    this.lastEventId = null;
    this.reconnectDelay = RECONNECT_BASE_MS;
  }
}

export const wsClient = new WsClient();
