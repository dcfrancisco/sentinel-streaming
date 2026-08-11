#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "Usage: SENTINEL_API_TOKEN=... $0 <base-url> <source-id> <output-dir>" >&2
  exit 2
}

[[ $# -eq 3 ]] || usage
BASE_URL="${1%/}"
SOURCE_ID="$2"
OUTPUT_DIR="$3"
TOKEN="${SENTINEL_API_TOKEN:-}"
[[ -n "$TOKEN" ]] || { echo "SENTINEL_API_TOKEN is required and is never written to evidence." >&2; exit 2; }

mkdir -p "$OUTPUT_DIR"
SAFE_ID="${SOURCE_ID//[^A-Za-z0-9_.-]/_}"
SOURCE_PATH="${SOURCE_ID// /%20}"

request() {
  local endpoint="$1"
  curl --fail --silent --show-error --max-time 20 \
    -H "Authorization: Bearer ${TOKEN}" \
    -H "Accept: application/json" \
    "${BASE_URL}${endpoint}"
}

request "/api/v1/version" > "${OUTPUT_DIR}/version.json"
request "/api/v1/sources/${SOURCE_PATH}" > "${OUTPUT_DIR}/source-${SAFE_ID}.json"
request "/api/v1/sources/${SOURCE_PATH}/capabilities" > "${OUTPUT_DIR}/capabilities-${SAFE_ID}.json" || true
request "/api/v1/sources/${SOURCE_PATH}/media" > "${OUTPUT_DIR}/media-${SAFE_ID}.json" || true
request "/api/v1/media/health" > "${OUTPUT_DIR}/media-gateway-health.json"
request "/api/v1/sources/${SOURCE_PATH}/playback" > "${OUTPUT_DIR}/playback-${SAFE_ID}.json" || true

cat > "${OUTPUT_DIR}/MANIFEST.md" <<EOF
# Physical certification capture

- Evidence class: `PHYSICAL_DEVICE`
- Source label: `${SAFE_ID}`
- Capture date (UTC): $(date -u +%Y-%m-%dT%H:%M:%SZ)
- Base URL: omitted from evidence for privacy
- Token: not stored

Files contain normalized Sentinel API responses only. Review and sanitize them
before committing. Complete [TEMPLATE.md](../../docs/certification/TEMPLATE.md)
with the physical test observations; this capture does not certify PTZ or
perform any destructive operation.
EOF

echo "Captured sanitized normalized API evidence in ${OUTPUT_DIR}."
