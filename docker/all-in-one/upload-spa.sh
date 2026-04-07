#!/bin/sh
set -e

SPA_DIR="/usr/share/burst/html"
ENDPOINT="${BURST_S3_ENDPOINT:?BURST_S3_ENDPOINT is required}"
BUCKET="${BURST_SPA_BUCKET:-burst-spa}"
REGION="${BURST_S3_REGION:-us-east-1}"
ACCESS_KEY="${BURST_S3_ACCESS_KEY_ID:?BURST_S3_ACCESS_KEY_ID is required}"
SECRET_KEY="${BURST_S3_SECRET_ACCESS_KEY:?BURST_S3_SECRET_ACCESS_KEY is required}"

# RustFS requires x-amz-content-sha256 header; curl <8.0 doesn't send it
# automatically with --aws-sigv4. UNSIGNED-PAYLOAD bypasses body hashing.
S3_HASH="-H x-amz-content-sha256:UNSIGNED-PAYLOAD"

s3_curl() {
  curl --aws-sigv4 "aws:amz:${REGION}:s3" \
    -u "${ACCESS_KEY}:${SECRET_KEY}" \
    ${S3_HASH} "$@"
}

echo "Waiting for S3 storage..."
until curl -so /dev/null "${ENDPOINT}/" 2>/dev/null; do
  sleep 1
done

echo "Creating bucket ${BUCKET}..."
s3_curl -sf -X PUT "${ENDPOINT}/${BUCKET}" -o /dev/null 2>/dev/null || true

echo "Uploading SPA files to ${BUCKET}..."
find "${SPA_DIR}" -type f | while read -r file; do
  key="${file#${SPA_DIR}/}"
  s3_curl -sf -T "${file}" "${ENDPOINT}/${BUCKET}/${key}" -o /dev/null
  echo "  ${key}"
done

echo "SPA upload complete."
