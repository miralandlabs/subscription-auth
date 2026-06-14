#!/usr/bin/env bash
# Apply subscription-auth schema (idempotent). Requires DATABASE_URL.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [[ -z "${DATABASE_URL:-}" ]]; then
  echo "Set DATABASE_URL (Preview Postgres connection string)"
  exit 1
fi
command -v psql >/dev/null || { echo "psql required"; exit 1; }
echo "Applying migrations/init.sql ..."
psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f "${ROOT}/migrations/init.sql"
echo "Schema apply complete."
