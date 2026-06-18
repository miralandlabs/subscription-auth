#!/usr/bin/env bash
# Smoke test subscription-auth Preview (or any deployment).
# Usage: SUBSCRIPTION_AUTH_BASE_URL=https://preview.auth.ipay.sh ./scripts/smoke-preview.sh
set -euo pipefail

BASE_URL="${SUBSCRIPTION_AUTH_BASE_URL:-https://preview.auth.ipay.sh}"
BASE_URL="${BASE_URL%/}"

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "missing command: $1"
    exit 1
  }
}

require_cmd curl
require_cmd jq

curl_retry() {
  local attempt max=5 delay=2
  for attempt in $(seq 1 "$max"); do
    if curl "$@"; then
      return 0
    fi
    if [[ "$attempt" -lt "$max" ]]; then
      echo "curl failed (attempt ${attempt}/${max}), retrying in ${delay}s..." >&2
      sleep "$delay"
    fi
  done
  return 1
}

echo "Checking /health on ${BASE_URL} ..."
HEALTH_JSON="$(mktemp)"
HEALTH_STATUS="$(curl_retry -sS -o "$HEALTH_JSON" -w "%{http_code}" "${BASE_URL}/health")"
if [[ "$HEALTH_STATUS" != "200" ]]; then
  echo "FAIL: /health returned HTTP ${HEALTH_STATUS}"
  cat "$HEALTH_JSON"
  exit 1
fi

jq -e '.status == "ok"' "$HEALTH_JSON" >/dev/null || {
  echo "FAIL: /health status not ok"
  cat "$HEALTH_JSON"
  exit 1
}

DB="$(jq -r '.db // empty' "$HEALTH_JSON")"
DB_PING="$(jq -r '.db_ping // empty' "$HEALTH_JSON")"
if [[ "$DB" != "configured" ]]; then
  echo "FAIL: db=${DB} (expected configured — set DATABASE_URL on Vercel)"
  cat "$HEALTH_JSON"
  exit 1
fi
if [[ "$DB_PING" != "ok" ]]; then
  echo "FAIL: db_ping=${DB_PING} (apply migrations/init.sql or check DATABASE_URL)"
  cat "$HEALTH_JSON"
  exit 1
fi
echo "OK: /health (db configured, db_ping ok)"
cat "$HEALTH_JSON"
echo ""

echo "Checking /.well-known/jwks.json on ${BASE_URL} ..."
JWKS_JSON="$(mktemp)"
JWKS_STATUS="$(curl_retry -sS -o "$JWKS_JSON" -w "%{http_code}" "${BASE_URL}/.well-known/jwks.json")"
if [[ "$JWKS_STATUS" != "200" ]]; then
  echo "FAIL: /.well-known/jwks.json returned HTTP ${JWKS_STATUS}"
  echo "Hint: set SUBSCRIPTION_AUTH_RSA_PRIVATE_KEY_PEM and SUBSCRIPTION_AUTH_KEY_ID on Vercel"
  cat "$JWKS_JSON"
  exit 1
fi

KEY_COUNT="$(jq '.keys | length' "$JWKS_JSON")"
if [[ "$KEY_COUNT" -lt 1 ]]; then
  echo "FAIL: JWKS has no keys"
  cat "$JWKS_JSON"
  exit 1
fi
echo "OK: JWKS (${KEY_COUNT} key(s))"
jq '.keys[] | {kid, alg, use}' "$JWKS_JSON"
echo ""
echo "Smoke test passed."
