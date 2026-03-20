# ── Builder stage ─────────────────────────────────────────────────────────────
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

# Default storage directory
RUN mkdir -p /var/lib/burst/uploads && chown 1000:1000 /var/lib/burst/uploads
VOLUME /var/lib/burst/uploads

USER 1000
EXPOSE 3000 3001

ENTRYPOINT ["burst"]
