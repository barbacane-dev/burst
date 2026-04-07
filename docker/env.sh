#!/bin/sh
# Generate /usr/share/burst/html/env.js from OIDC_* environment variables.
# Runs at container startup before SPA files are uploaded to S3.

cat > /usr/share/burst/html/env.js <<EOF
window.__BURST_ENV__ = {
  OIDC_AUTHORITY: "${OIDC_AUTHORITY:-}",
  OIDC_CLIENT_ID: "${OIDC_CLIENT_ID:-}",
  OIDC_REDIRECT_URI: "${OIDC_REDIRECT_URI:-}",
  OIDC_SCOPE: "${OIDC_SCOPE:-}",
  LOGIN_LOCAL: "${LOGIN_LOCAL:-true}",
};
EOF
