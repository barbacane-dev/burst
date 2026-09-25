#!/bin/sh
# Writes env.js, the SPA's runtime configuration, from the OIDC_* and
# LOGIN_LOCAL environment variables. BURST_ENV_JS is where it goes: the
# all-in-one image serves the SPA from /usr/share/burst/html, the nginx image
# from /usr/share/nginx/html.
#
# LOGIN_LOCAL is off by default: the form has no backend until local accounts
# land (ADR-015). Set LOGIN_LOCAL=true to show it anyway.
set -eu

target="${BURST_ENV_JS:-/usr/share/burst/html/env.js}"

cat > "$target" <<JS
window.__BURST_ENV__ = {
  OIDC_AUTHORITY: "${OIDC_AUTHORITY:-}",
  OIDC_CLIENT_ID: "${OIDC_CLIENT_ID:-}",
  OIDC_REDIRECT_URI: "${OIDC_REDIRECT_URI:-}",
  OIDC_SCOPE: "${OIDC_SCOPE:-}",
  LOGIN_LOCAL: "${LOGIN_LOCAL:-false}",
};
JS
