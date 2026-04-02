#!/bin/sh
# Generate /usr/share/nginx/html/env.js from OIDC_* environment variables.
# Runs at container startup before nginx serves the SPA.

cat > /usr/share/nginx/html/env.js <<EOF
window.__BURST_ENV__ = {
  OIDC_AUTHORITY: "${OIDC_AUTHORITY:-}",
  OIDC_CLIENT_ID: "${OIDC_CLIENT_ID:-}",
  OIDC_REDIRECT_URI: "${OIDC_REDIRECT_URI:-}",
  OIDC_SCOPE: "${OIDC_SCOPE:-}",
};
EOF
