# subscription-auth

Hosted subscription JWT service: RS256 issue/revoke, JWKS, wallet-signed service registration, revocation delta feed.

x402 sellers use **Tier B** via [`@pr402/subscription-seller`](https://www.npmjs.com/package/@pr402/subscription-seller) + `SUBSCRIPTION_MODE=tier-b`.

**Seller guide:** [docs/SUBSCRIPTION_AUTH_FOR_SELLERS.md](docs/SUBSCRIPTION_AUTH_FOR_SELLERS.md)

**Optional entitlement pattern (not a pr402 rail):** [docs/YIELD_QUALIFIED_SUBSCRIPTION.md](docs/YIELD_QUALIFIED_SUBSCRIPTION.md) — hold ≥ threshold in yield/RWA mint → JWT extend without `exact` settle.

---

## Seller integration (Tier B)

1. Deploy this service (below) or use a hosted instance (e.g. `https://preview.auth.ipay.sh` for devnet tests).
2. **Register once:** `node scripts/register-service.mjs --keypair … --service-id … --service-url …`
3. On your seller: `SUBSCRIPTION_MODE=tier-b`, auth URLs, `SUBSCRIPTION_AUTH_SERVICE_ID`, merchant secret key.
4. Flow unchanged for buyers: pay on `/subscribe` → Bearer on data routes.

Payment (`402` + pr402) stays on **your seller** — this service only signs JWTs after you settle.

---

## Deploy (Vercel + Postgres)

1. Postgres (Neon/Supabase); apply schema:
   ```bash
   psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f migrations/init.sql
   ```
2. RSA keypair:
   ```bash
   openssl genrsa -out private.pem 2048
   ```
3. Vercel env: `DATABASE_URL`, `SUBSCRIPTION_AUTH_HMAC_SECRET`, `SUBSCRIPTION_AUTH_RSA_PRIVATE_KEY_PEM`, `SUBSCRIPTION_AUTH_KEY_ID`, `SUBSCRIPTION_AUTH_ISS`
4. Deploy → `GET /health`, `GET /.well-known/jwks.json`

---

## API (v1)

| Method | Path | Auth |
|--------|------|------|
| GET | `/health` | — |
| GET | `/.well-known/jwks.json` | — |
| GET | `/v1/services/{wallet}/challenge` | — |
| POST | `/v1/services/{wallet}/register` | Wallet sig |
| POST | `/v1/services/{wallet}/update` | Wallet sig |
| POST | `/v1/services/{wallet}/retire` | Wallet sig |
| POST | `/v1/tokens/issue` | Wallet sig |
| POST | `/v1/tokens/revoke` | Wallet sig |
| POST | `/v1/tokens/introspect` | Bearer token |
| GET | `/v1/revocations?service_id=&since=` | — |

Challenge domain: `x402 subscription auth v1` — see `src/challenge_auth.rs`.

---

## Revocation

Sellers poll `GET /v1/revocations` every ~60s (fail-open). Revocation may take up to one poll interval.

---

## Testing

Scripts in [`scripts/`](scripts/) — optional `scripts/.env.local` from [`scripts/env.example`](scripts/env.example).

```bash
# Health + JWKS (retries on flaky TLS)
./scripts/smoke-preview.sh

# npm SDK smoke
node scripts/verify-npm-package.mjs

# Register demo seller (one-time)
node scripts/register-service.mjs --keypair ../../demo-wallets/seller-keypair.json

# Auth-only: issue → JWKS verify → revoke (no pr402 payment)
node scripts/e2e-tier-b-auth.mjs --keypair ../../demo-wallets/seller-keypair.json

# Read-only hold-qualification spike (no npm deps)
node scripts/check-hold-qualification.mjs --wallet <pubkey> --min-usd 300
```

**Full stack** (pr402 Preview + local seller + buyer): [starter Tier B example](https://github.com/miralandlabs/x402-subscription-starter/tree/main/examples/tier-b-preview-e2e) — run seller and buyer scripts in two terminals.

---

## License

MIT
