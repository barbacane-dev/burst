#!/bin/sh
set -e

# Burst, the S3 sidecar and RustFS are loopback or private-network upstreams;
# the plugin SSRF guard blocks that egress unless this is set.
export BARBACANE_ALLOW_INTERNAL_EGRESS=true

# ── 1. Generate runtime env.js ───────────────────────────────────────────────
/etc/burst/env.sh

# ── 2. Upload SPA to S3 (directly to RustFS) ────────────────────────────────
/etc/burst/upload-spa.sh

# ── 3. Start background services ────────────────────────────────────────────
/usr/local/bin/barbacane serve \
  --artifact /etc/barbacane/burst-s3.bca \
  --listen 127.0.0.1:8081 \
  --admin-bind off \
  --allow-plaintext-upstream \
  --max-body-size 104857600 &

RUST_LOG="${RUST_LOG:-info}" /usr/local/bin/burst /etc/burst/burst.toml &

# ── 4. Wait for Burst to be ready before starting the public gateway ────────
echo "Waiting for Burst API..."
until curl -sf http://127.0.0.1:3001/health/live >/dev/null 2>&1; do
  sleep 1
done

# ── 5. Start public gateway (PID 1, receives signals) ──────────────────────
exec /usr/local/bin/barbacane serve \
  --artifact /etc/barbacane/burst-api.bca \
  --listen 0.0.0.0:8080 \
  --admin-bind off \
  --allow-plaintext-upstream \
  --max-body-size 10485760
