# subscription-auth

General-purpose hosted subscription JWT signing oracle: RS256 mint/revoke, JWKS, wallet-signed service registration, revocation delta feed.

x402/pr402 sellers integrate via [`@pr402/subscription-seller`](../x402-subscription-seller) Tier B (`SUBSCRIPTION_MODE=tier-b` in starter).

## Deploy (Vercel + Postgres)

1. Create Postgres (Neon/Supabase); apply schema:
   ```bash
   psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f migrations/init.sql
   ```
2. Generate RSA keypair:
   ```bash
   openssl genrsa -out private.pem 2048
   ```
3. Set Vercel env: `DATABASE_URL`, `SUBSCRIPTION_AUTH_HMAC_SECRET`, `SUBSCRIPTION_AUTH_RSA_PRIVATE_KEY_PEM`, `SUBSCRIPTION_AUTH_KEY_ID`, `SUBSCRIPTION_AUTH_ISS`
4. Deploy; verify `GET /health`, `GET /.well-known/jwks.json`

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

See [docs/subscription-jwt-auth-challenge-spec.md](../docs/subscription-jwt-auth-challenge-spec.md).

## Revocation UX

Sellers poll `GET /v1/revocations` every ~60s (fail-open). Revocation may take up to one poll interval.

## License

MIT
