# ── Runtime stage ─────────────────────────────────────────────────────────────
FROM gcr.io/distroless/cc-debian12:nonroot

ARG TARGETARCH

COPY bin/${TARGETARCH}/burst /usr/local/bin/burst

EXPOSE 3000 3001

ENTRYPOINT ["burst"]
