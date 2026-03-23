# ADR-013: Frontend Architecture

**Status:** Accepted
**Date:** 2026-03-06

## Context

Per [ADR-003](003-technology-stack.md), Burst's frontend uses React, TypeScript, Vite, and Tailwind CSS — the same stack as Barbacane's control plane UI. Per [ADR-004](004-real-time-architecture.md), the frontend maintains a persistent WebSocket connection for real-time events. Per [ADR-008](008-crate-architecture.md), the frontend is a standard Vite project in `ui/`, built separately and served by nginx (`docker/Dockerfile.nginx`).

Barbacane's UI is an admin dashboard — CRUD forms, tables, and status pages. Burst's UI is a messaging application — fundamentally different in interaction patterns:

- **Always-on real-time.** A messaging app is event-driven. The WebSocket is the primary data channel, not REST.
- **High-frequency updates.** New messages, typing indicators, presence changes — the UI must handle many events per second without performance degradation.
- **Large data sets.** Channels can have thousands of messages. The client must manage a normalised, memory-efficient message store with virtualised rendering.
- **Offline resilience.** Users expect the app to work during brief connectivity interruptions and catch up seamlessly on reconnect.

We can share tooling and conventions with Barbacane (Vite, Tailwind, TanStack Query, component patterns) while adapting the architecture for messaging-specific needs.

## Decision

### Project structure

```
ui/
├── src/
│   ├── main.tsx                    # Entry point, router, providers
│   ├── index.css                   # Tailwind imports, global styles
│   ├── components/
│   │   ├── ui/                     # Design system primitives (Button, Input, Avatar, Badge, etc.)
│   │   ├── layout/                 # App shell (Sidebar, ChannelHeader, MainLayout)
│   │   ├── channel/                # Channel list, channel item, create channel
│   │   ├── message/                # Message bubble, message list, composer, thread
│   │   ├── user/                   # User profile, presence indicator, member list
│   │   └── search/                 # Search input, results, filters
│   ├── hooks/                      # Custom React hooks
│   │   ├── use-websocket.ts        # WebSocket connection lifecycle
│   │   ├── use-messages.ts         # Message loading, pagination, optimistic updates
│   │   ├── use-channels.ts         # Channel list, membership
│   │   ├── use-presence.ts         # Online/offline/away status
│   │   ├── use-typing.ts           # Typing indicators
│   │   └── use-search.ts           # Search with debounce
│   ├── lib/
│   │   ├── api/                    # REST API client (TanStack Query)
│   │   │   ├── client.ts           # fetch wrapper, error handling
│   │   │   └── types.ts            # API types (generated from OpenAPI spec)
│   │   ├── ws/                     # WebSocket client
│   │   │   ├── client.ts           # Connection, reconnection, event dispatch
│   │   │   └── types.ts            # Event envelope types
│   │   ├── store/                   # Client-side state
│   │   │   ├── messages.ts         # Normalised message store
│   │   │   └── channels.ts         # Channel state
│   │   ├── auth/                   # Auth context, token management
│   │   │   # Token storage strategy: access token in memory only (module-level variable),
│   │   │   # refresh token in httpOnly cookie (not readable by JS). On page load, the app
│   │   │   # calls POST /auth/refresh (cookie sent automatically) to obtain a fresh access
│   │   │   # token before connecting. The WebSocket first frame uses the in-memory token.
│   │   │   # localStorage is never used for tokens — XSS cannot exfiltrate what JS cannot read.
│   │   └── utils.ts                # cn(), relativeTime(), formatFileSize()
│   └── pages/                      # Route-level components
│       ├── login.tsx
│       ├── channel.tsx
│       ├── thread.tsx
│       ├── search.tsx
│       └── settings.tsx
├── public/                          # Static assets, favicon
├── index.html
├── package.json
├── tsconfig.json
├── vite.config.ts
├── vitest.config.ts
└── playwright.config.ts
```

### Shared patterns with Barbacane

| Pattern | Barbacane | Burst | Shared? |
|---------|-----------|-------|---------|
| React + TypeScript | Yes | Yes | Same |
| Vite build | Yes | Yes | Same |
| Tailwind CSS | Yes | Yes | Same |
| `cn()` via clsx + tailwind-merge | Yes | Yes | Same |
| TanStack Query for REST | Yes | Yes | Same |
| `components/ui/` primitives | Yes | Yes | Same convention |
| react-router-dom | Yes | Yes | Same |
| Vitest + Playwright | Yes | Yes | Same |
| lucide-react icons | Yes | Yes | Same |
| ProblemDetails error handling | Yes | Yes | Same `ApiError` class |

Contributors moving between Barbacane and Burst see the same toolchain, the same file organisation conventions, and the same testing patterns.

### What's different: the WebSocket-first model

Barbacane's UI is request/response driven — click a button, fetch data, display. Burst's UI is event-driven — the WebSocket pushes most state changes.

#### Data flow

```
REST API (TanStack Query)          WebSocket
        │                              │
        │  Initial load:               │  Real-time updates:
        │  - Channel list              │  - New messages
        │  - Message history           │  - Typing indicators
        │  - User profile              │  - Presence changes
        │  - Search results            │  - Message edits/deletes
        │                              │  - Reactions
        │                              │  - Channel updates
        ▼                              ▼
    ┌──────────────────────────────────────┐
    │         Client-side store            │
    │  (normalised messages + channels)    │
    └──────────────────────────────────────┘
                    │
                    ▼
              React components
```

- **REST** is for initial data loading and user-initiated actions (send message, create channel, upload file, search).
- **WebSocket** is for all real-time updates. When a WebSocket event arrives, it updates the client-side store directly. TanStack Query caches are invalidated or updated optimistically.

**Seam between TanStack Query and the store:** TanStack Query is the fetch transport; the normalised store is the render source of truth. On successful fetch, query `onSuccess` handlers write into the store — they do not maintain a parallel cache for rendered messages. As a result, message queries use `staleTime: Infinity`: background refetches are disabled because the WebSocket is the live update channel. This eliminates the dual-source-of-truth problem where a background refetch could overwrite store state that is ahead of the server response.

### WebSocket client

The WebSocket client (`lib/ws/client.ts`) handles:

- **Connection lifecycle.** Connect on login, reconnect on disconnect with exponential backoff (1s, 2s, 4s, 8s, max 30s). Per [ADR-004](004-real-time-architecture.md), the first frame sends the auth token.
- **Event dispatch.** Incoming events are parsed and dispatched to registered handlers (hooks). Each event type (`message.created`, `typing.start`, `presence.update`, etc.) has a typed handler.
- **Gap-fill.** On reconnect, the client sends its last received event ID. The server sends missed events before resuming the live stream. This is transparent to UI components.
- **Heartbeat.** Periodic ping/pong (30s interval) to detect dead connections early.

**Implementation constraint: the client is a singleton, not a hook return.** `lib/ws/client.ts` exports a class instance created once on login and exposed via React context. Hooks receive the stable context reference — they do not create or own the connection. This prevents two common bugs: (1) re-creating the connection on re-render, and (2) stale closure capture when handlers close over state.

Event handlers use a stable-ref pattern to avoid re-subscribing on every render:

```typescript
// hooks/use-ws-event.ts
export function useWsEvent<T extends BurstEventType>(
  type: T,
  handler: (e: BurstEventMap[T]) => void
) {
  const ws = useWsClient()           // stable context ref — never changes identity
  const handlerRef = useRef(handler)
  useLayoutEffect(() => { handlerRef.current = handler }) // keep ref fresh without re-subscribing

  useEffect(() => {
    return ws.on(type, (e) => handlerRef.current(e))
  }, [ws, type])                     // handler intentionally excluded — ref handles freshness
}
```

### Client-side store

A lightweight normalised store for messages and channels. Not Redux, not Zustand — a purpose-built store using React context + `useReducer` for the specific needs of a messaging app:

- **Normalised messages.** Messages are stored in a `Map<messageId, Message>` with per-channel ordered ID lists. This enables O(1) lookups by ID (for edits, deletes, reactions) and efficient ordered rendering per channel.
- **Windowed retention.** Only the most recent N messages per channel are kept in memory (e.g., 500). Scrolling up triggers REST pagination to load older messages. This bounds memory usage regardless of channel history size.
- **Optimistic updates.** When the user sends a message, it appears immediately in the UI with a pending state. When the server confirms (via WebSocket event), the pending message is replaced with the real one. On failure, the message shows an error state with retry.

### TanStack Query usage

TanStack Query manages REST API interactions, consistent with Barbacane's UI:

- **Channel list**: `useQuery(['channels'])` — fetched on login, invalidated on `channel.created` / `channel.archived` WebSocket events.
- **Message history**: `useInfiniteQuery(['messages', channelId])` — cursor-based pagination, new pages prepended on scroll-up.
- **User profile**: `useQuery(['user', userId])` — cached, rarely invalidated.
- **Search**: `useQuery(['search', query])` — debounced, short stale time.

WebSocket events trigger targeted cache invalidation (`queryClient.invalidateQueries`) rather than full refetches. For high-frequency updates (new messages), the store is updated directly without going through TanStack Query.

### Virtualised message list

The message list uses **`react-virtuoso`** for virtualised rendering. `@tanstack/react-virtual` was considered but rejected: it is a headless primitive that requires building scroll management from scratch. For a chat-style list, the hard parts — scroll-to-bottom on new messages, prepend-on-scroll-up without losing position, variable-height item measurement — are solved problems in `react-virtuoso` (`followOutput`, `firstItemIndex` for prepend, automatic height measurement). The bespoke scroll logic saved is not worth writing.

- Only visible messages (plus a buffer) are rendered in the DOM.
- `followOutput="smooth"` handles scroll-to-bottom when the user is at the bottom; new messages arriving while scrolled up show a "new messages" indicator instead of jumping.
- Older messages are prepended via `firstItemIndex` adjustment — `react-virtuoso` handles position stability during prepend without scroll jump.
- Variable-height messages (text, images, embeds) are measured and cached by the library.

### Component design

#### UI primitives (`components/ui/`)

Reusable, unstyled (or minimally styled) components using Tailwind + class-variance-authority (same as Barbacane):

- `Button`, `Input`, `Textarea`, `Select`
- `Avatar` (with presence dot overlay)
- `Badge`, `Tooltip`, `Dialog`, `DropdownMenu`
- `Spinner`, `EmptyState`, `ErrorBoundary`

These are the same component patterns as Barbacane's UI. If a shared component library emerges later, these are the extraction candidates.

#### Domain components

- **`MessageBubble`** — renders a single message: content (Markdown), author, timestamp, reactions, attachments, thread reply count.
- **`MessageComposer`** — text input with Markdown preview, file attachment, emoji picker, mentions autocomplete.
- **`ChannelSidebar`** — channel list grouped by category (channels, DMs), with unread counts and presence indicators.
- **`ThreadPanel`** — side panel showing thread replies, reusing `MessageBubble` and `MessageComposer`.
- **`MemberList`** — channel member list with presence indicators and role badges.
- **`SearchResults`** — message search results with highlighted matches and channel context.

### Routing

```
/login                          → Login page
/channels/:slug                 → Channel view (message list + composer)
/channels/:slug/threads/:id     → Channel view with thread panel open
/dm/:userId                     → Direct message (resolves to DM channel)
/search?q=...                   → Search results
/settings                       → User settings (profile, notifications)
/admin                          → Admin panel (users, channels, webhooks)
```

Channel slugs are used in URLs for readability (`/channels/engineering` instead of `/channels/uuid`).

### Markdown rendering

Messages support a subset of Markdown:

- **Bold**, *italic*, ~~strikethrough~~, `inline code`
- Code blocks with syntax highlighting (via `shiki`, same library Barbacane uses)
- Links (auto-linked URLs)
- Mentions (`@username`) — rendered as clickable, highlighted spans
- Emoji shortcodes (`:thumbsup:` → 👍, custom emoji from `custom_emojis` table)
- Block quotes

Markdown is rendered client-side. The server stores raw Markdown text.

**Rendering pipeline (order is security-sensitive):**

```
raw markdown → remark-parse → remark-rehype → @shikijs/rehype → rehype-sanitize → html string → dangerouslySetInnerHTML
```

Sanitisation via `rehype-sanitize` runs **after** `@shikijs/rehype`, not before. Reversing the order either strips shiki's `<span>` output (breaking highlighting) or leaves unsanitised content in the DOM. The `rehype-sanitize` schema must explicitly allow `span`, `pre`, and `code` elements with the class attributes shiki emits.

Raw HTML passthrough in the Markdown parser must be disabled (`allowDangerousHtml: false` on `remark-rehype`). Users cannot inject HTML via message content.

### Accessibility

- Keyboard navigation: arrow keys to move between messages, Enter to reply, Escape to close panels.
- ARIA roles: `role="log"` for message list, `role="textbox"` for composer, `role="navigation"` for sidebar.
- Screen reader support: new message announcements via an `aria-live="polite"` region. **Do not place `aria-live` directly on the message list.** In an active channel, every incoming message would trigger a screen reader announcement, which is disruptive. The live region is a separate off-screen element; announcements are debounced and scoped to mentions and DMs unless the user has focus in the channel.
- Focus management: focus moves to composer after channel switch, returns to message list after sending.

### Notifications

- **Browser notifications** (via Notification API) for mentions and DMs when the tab is not focused.
- **Unread counts** displayed in the channel sidebar and document title (`(3) Burst`).
- **Notification preferences** per channel (all, mentions, nothing) — stored server-side in `channel_members.notify`, respected client-side.

### Theming

- **Light and dark mode** supported via Tailwind's `dark:` variant.
- Theme preference stored in `localStorage`, respects system preference by default.
- Same approach as Barbacane's UI (`use-theme.ts` hook).

### Build and serving

Per [ADR-008](008-crate-architecture.md), the frontend is built separately:

```bash
cd ui && npm run build    # produces ui/dist/
```

In production, nginx serves `ui/dist/` at the root path and proxies `/api/*` and `/ws` to Barbacane (see `docker/Dockerfile.nginx` and `docker/nginx.conf`). The Burst binary is a pure API server — it does not serve static files.

In development, the Vite dev server proxies `/api/` and `/ws` to Barbacane for hot reload.

### What we deliberately avoided

- **No SSR.** A messaging app is a single-page application. Server-side rendering adds complexity with no benefit for an always-authenticated, WebSocket-connected app.
- **No global state library.** No Redux, no Zustand, no MobX. TanStack Query handles server state. The message store is a focused, purpose-built structure. React context handles the few pieces of truly global state (auth, theme, WebSocket connection).
- **No component library dependency.** No Radix, no shadcn/ui, no Material UI. Burst's UI primitives are built with Tailwind + CVA, same as Barbacane. This keeps the dependency tree small and styling consistent across projects.
- **No i18n in v1.** English only. Internationalisation can be added later with react-intl or similar, but it's premature complexity for a v1.

## Consequences

- **Shared toolchain with Barbacane.** Same React version, same build tool, same styling approach, same testing tools. Contributors work across both UIs without re-learning.
- **WebSocket-first is the right model for messaging** but adds complexity compared to a pure REST UI. The WebSocket client, reconnection logic, gap-fill, and optimistic updates are the most complex frontend code. This is inherent to the domain.
- **The normalised message store is custom code.** Off-the-shelf state libraries are designed for generic state. A messaging app needs specific data structures (ordered ID lists per channel, windowed retention, pending message states). Building this is more work upfront but avoids fighting a generic library later.
- **Virtualised rendering is essential.** Without it, channels with thousands of messages would freeze the browser. The trade-off is more complex scroll behaviour (position preservation, variable heights, scroll-to-bottom logic).
- **No component sharing with Barbacane yet.** Both projects have `components/ui/` with similar primitives. If the projects grow, extracting a shared `@barbacane/ui` package is a natural step. For now, copy-paste is acceptable — the components are small and the abstraction cost isn't justified.
