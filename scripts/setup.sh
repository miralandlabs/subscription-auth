#!/usr/bin/env bash
# scripts/setup.sh — Guided first-time setup for subscription-auth
#
# What this does:
#   1. Generates a 2048-bit RSA keypair  (private.pem)
#   2. Generates a random HMAC secret    (≥32 bytes)
#   3. Outputs a ready-to-paste .env block
#   4. Optionally adds all vars to Vercel via `vercel env add`
#
# Usage:
#   ./scripts/setup.sh                   # print .env block to stdout
#   ./scripts/setup.sh --vercel          # also push to Vercel (requires vercel CLI)
#   ./scripts/setup.sh --key-id mykey-2  # custom key_id (default: auth-key-1)
#   ./scripts/setup.sh --iss https://auth.example.com

set -euo pipefail

# ─── argument parsing ────────────────────────────────────────────────────────
PUSH_VERCEL=false
KEY_ID="auth-key-1"
ISS=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --vercel)    PUSH_VERCEL=true; shift ;;
    --key-id)    KEY_ID="$2"; shift 2 ;;
    --iss)       ISS="$2";    shift 2 ;;
    *) echo "Unknown argument: $1" >&2; exit 1 ;;
  esac
done

# ─── colours ─────────────────────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; CYAN='\033[0;36m'; RESET='\033[0m'
info()  { echo -e "${CYAN}[setup]${RESET} $*"; }
ok()    { echo -e "${GREEN}[ok]${RESET}   $*"; }
warn()  { echo -e "${YELLOW}[warn]${RESET} $*"; }
err()   { echo -e "${RED}[err]${RESET}  $*" >&2; }

# ─── prerequisites ────────────────────────────────────────────────────────────
for cmd in openssl awk; do
  if ! command -v "$cmd" &>/dev/null; then
    err "Required command not found: $cmd"
    exit 1
  fi
done

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

# ─── 1. RSA keypair ───────────────────────────────────────────────────────────
PEM_FILE="$ROOT_DIR/subscription-auth-private.pem"
if [[ -f "$PEM_FILE" ]]; then
  warn "Private key already exists at $PEM_FILE — reusing it."
  warn "Delete it and re-run to generate a fresh key."
else
  info "Generating 2048-bit RSA private key …"
  openssl genrsa -out "$PEM_FILE" 2048 2>/dev/null
  chmod 600 "$PEM_FILE"
  ok "Key written to $PEM_FILE"
fi

# Escape the PEM for a single-line env var (replace literal newlines with \n)
PEM_SINGLE_LINE="$(awk 'NF{printf "%s\\n",$0}' "$PEM_FILE")"

# ─── 2. HMAC secret ───────────────────────────────────────────────────────────
info "Generating HMAC secret (48 hex bytes) …"
HMAC_SECRET="$(openssl rand -hex 48)"
ok "HMAC secret generated."

# ─── 3. ISS default ───────────────────────────────────────────────────────────
if [[ -z "$ISS" ]]; then
  warn "No --iss provided. Using placeholder; update SUBSCRIPTION_AUTH_ISS before deploying."
  ISS="https://auth.example.com"
fi

# ─── 4. Print .env block ─────────────────────────────────────────────────────
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  Copy the following into your Vercel / .env file:"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
cat <<EOF
DATABASE_URL=postgresql://user:pass@host/db?sslmode=require
SUBSCRIPTION_AUTH_HMAC_SECRET=${HMAC_SECRET}
SUBSCRIPTION_AUTH_RSA_PRIVATE_KEY_PEM="${PEM_SINGLE_LINE}"
SUBSCRIPTION_AUTH_KEY_ID=${KEY_ID}
SUBSCRIPTION_AUTH_ISS=${ISS}
EOF
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# ─── 5. Optional: push to Vercel ─────────────────────────────────────────────
if [[ "$PUSH_VERCEL" == "true" ]]; then
  if ! command -v vercel &>/dev/null; then
    err "vercel CLI not found. Install with: npm i -g vercel"
    exit 1
  fi

  info "Pushing env vars to Vercel …"

  push_env() {
    local key="$1" value="$2"
    echo "$value" | vercel env add "$key" production --force 2>/dev/null \
      && ok "Set $key" \
      || warn "Failed to set $key — set it manually in Vercel dashboard."
  }

  # DATABASE_URL must be set manually (contains credentials)
  warn "DATABASE_URL not pushed — set it manually in the Vercel dashboard."

  push_env "SUBSCRIPTION_AUTH_HMAC_SECRET"         "$HMAC_SECRET"
  push_env "SUBSCRIPTION_AUTH_RSA_PRIVATE_KEY_PEM" "$PEM_SINGLE_LINE"
  push_env "SUBSCRIPTION_AUTH_KEY_ID"              "$KEY_ID"
  push_env "SUBSCRIPTION_AUTH_ISS"                 "$ISS"

  ok "Done. Run 'vercel deploy' to deploy with the new env vars."
else
  echo ""
  info "To push to Vercel automatically next time, run:"
  info "  ./scripts/setup.sh --vercel --iss https://auth.example.com"
fi

echo ""
info "Next steps:"
echo "  1. Set DATABASE_URL in Vercel dashboard"
echo "  2. Apply schema: psql \"\$DATABASE_URL\" -f migrations/init.sql"
echo "  3. Deploy to Vercel"
echo "  4. Verify: curl https://<your-deployment>/health"
echo "  5. Register your service:"
echo "       node scripts/register-service.mjs --keypair <path> --service-id api.myproduct.com"
