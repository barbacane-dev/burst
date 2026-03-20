# ── Frontend build stage ──────────────────────────────────────────────────────
FROM node:20-slim AS frontend

WORKDIR /ui
COPY ui/package.json ui/package-lock.json ./
RUN npm ci
COPY ui/ ./
ENV VITE_BASE_URL=/static/
RUN npx vite build --base /static/

# ── Rust build stage ─────────────────────────────────────────────────────────
FROM rust:1-bookworm AS builder

WORKDIR /build
COPY . .

# Build the release binary (sqlx offline mode — no DB required at build time)
ENV SQLX_OFFLINE=true
RUN cargo build --release --bin burst

# ── Runtime stage ─────────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/burst /usr/local/bin/burst
COPY --from=frontend /ui/dist /var/lib/burst/ui

# Default storage directory
RUN mkdir -p /var/lib/burst/uploads && chown -R 1000:1000 /var/lib/burst

USER 1000
EXPOSE 3000 3001

# Serve the built frontend from /var/lib/burst/ui
ENV BURST_SERVER_STATIC_DIR=/var/lib/burst/ui

ENTRYPOINT ["burst"]
