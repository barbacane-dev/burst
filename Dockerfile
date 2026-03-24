# ── Runtime stage ─────────────────────────────────────────────────────────────
FROM debian:bookworm-slim

ARG TARGETARCH

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

COPY bin/${TARGETARCH}/burst /usr/local/bin/burst
RUN chmod +x /usr/local/bin/burst

# Default storage directory
RUN mkdir -p /var/lib/burst/uploads && chown -R 1000:1000 /var/lib/burst

USER 1000
EXPOSE 3000 3001

ENTRYPOINT ["burst"]
