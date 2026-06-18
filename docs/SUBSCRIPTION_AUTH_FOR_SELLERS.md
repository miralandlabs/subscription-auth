# Subscription JWT auth — seller guide

**Pay gate stays on your seller.** x402/pr402 settles on `POST /api/v1/subscribe` only. JWT auth on data routes is a separate choice.

**Related:** [SUBSCRIPTION_PATTERN.md](https://github.com/miralandlabs/x402/blob/master/SUBSCRIPTION_PATTERN.md) (x402 hub) covers the **wire contract** — endpoints, 402 body, JWT claims, rate limits, buyer behavior. **This doc** covers **how to pick and configure** Tier A vs Tier B auth.

---

## Pick Tier A or Tier B

| | **Tier A — local JWT** | **Tier B — hosted auth** |
|---|------------------------|---------------------------|
| **When** | Single seller, fastest fork | Multi-seller, central revoke, RS256/JWKS |
| **Sign JWT** | You (`JWT_SECRET`, HS256) | [subscription-auth](../README.md) (RS256) |
| **Verify JWT** | Your server | Your server via JWKS |
| **Revoke early** | Your DB (`StrictStore`) | Auth feed poll (~60s, fail-open) |
| **Register auth service** | No | **Yes — once at deploy** |
| **Starter default** | Yes | `SUBSCRIPTION_MODE=tier-b` |

Both use the same subscribe/data-route contract in [SUBSCRIPTION_PATTERN.md](https://github.com/miralandlabs/x402/blob/master/SUBSCRIPTION_PATTERN.md).

---

## Tier A — local JWT (default)

**Best for:** fork [x402-subscription-starter](https://github.com/miralandlabs/x402-subscription-starter) and ship.

```bash
cp .env.example .env
openssl rand -hex 32   # → JWT_SECRET
# Set MERCHANT_WALLET, X402_PAY_TO, X402_ACCEPTS_EXTRA_JSON (see x402-seller-starter find-payto)
npm install && npm run dev
```

**Env (minimum):**

| Variable | Purpose |
|----------|---------|
| `JWT_SECRET` | HS256 signing key |
| `FACILITATOR_BASE_URL` | pr402 host (`https://preview.ipay.sh` devnet) |
| `MERCHANT_WALLET` | Seller identity in `accepts[].extra` |
| `X402_*` | Fallback pricing when SQLite has no tier row |

**Checklist:**

- [ ] x402 gate **only** on `/api/v1/subscribe`
- [ ] Data routes: `Authorization: Bearer` only
- [ ] Return `persistenceHint` on subscribe success
- [ ] Pick revocation: `StrictStore` (recommended) / `LenientStore` / `NoopStore`

SDK: [`@pr402/subscription-seller`](https://www.npmjs.com/package/@pr402/subscription-seller)

---

## Tier B — hosted auth

**Best for:** RS256 tokens, shared revocation, or many sellers under one auth deployment.

### 1. Deploy subscription-auth

Postgres + Vercel. See [README.md](../README.md).

Verify:

```bash
./scripts/smoke-preview.sh
```

### 2. Register your service (once)

Wallet-signed. Pick a stable `service_id` (e.g. `api.myproduct.com`).

```bash
node scripts/register-service.mjs \
  --keypair /path/to/seller-keypair.json \
  --service-id api.myproduct.com \
  --service-url https://api.myproduct.com
```

### 3. Configure the seller

```bash
SUBSCRIPTION_MODE=tier-b
SUBSCRIPTION_AUTH_BASE_URL=https://preview.auth.ipay.sh   # or your deployment
SUBSCRIPTION_AUTH_ISS=https://preview.auth.ipay.sh
SUBSCRIPTION_AUTH_SERVICE_ID=api.myproduct.com
SUBSCRIPTION_AUTH_MERCHANT_SECRET_KEY=<base58_64_byte_secret>
REVOCATION_POLL_INTERVAL_SEC=60
```

After x402 settle on `/subscribe`, the seller calls auth **`POST /v1/tokens/issue`** (wallet-signed) and returns the RS256 JWT.

### 4. Test

**Auth-only (no USDC):**

```bash
node scripts/e2e-tier-b-auth.mjs --keypair /path/to/seller-keypair.json
```

**Full stack** (pr402 + local seller + buyer): [starter Tier B example](https://github.com/miralandlabs/x402-subscription-starter/tree/main/examples/tier-b-preview-e2e) — run seller and buyer in two terminals.

---

## What is the same in Tier A and Tier B

| Step | Owner |
|------|--------|
| `402` + `accepts[]` on `/subscribe` | Seller |
| pr402 `verify` → `settle` | Seller → facilitator |
| JWT on data routes | Seller validates (local or JWKS) |
| Rate limits | Seller |

Buyers pay once per window and save the JWT locally until `exp`.

---

## Buyer payment proof (pr402 1.2)

Custom buyers (not [x402-subscription-client](https://github.com/miralandlabs/x402-subscription-client)):

1. `POST …/build-exact-payment-tx` → sign `VersionedTransaction`
2. `GET …/facilitator/capabilities` → `supported.kinds[scheme=exact].extra`
3. Merge rail `extra` into **`paymentPayload.accepted.extra`** and **`paymentRequirements.extra`**
4. Set signed tx on `paymentPayload.payload.transaction`
5. Send JSON as `PAYMENT-SIGNATURE` header

The subscription client handles this in `pr402-exact-flow.ts`.

---

## Troubleshooting Preview

| Symptom | Fix |
|---------|-----|
| `SSL_ERROR_SYSCALL` on curl but browser OK | Local proxy fake-IP (`198.18.0.x`); retry or bypass `*.ipay.sh` |
| Verify `missing field feePayer` | Verify body missing capabilities rail `extra` — see above |

Scripts retry fetch/curl 5× with 2s backoff.

---

## References

| Doc | Use |
|-----|-----|
| [SUBSCRIPTION_PATTERN.md](https://github.com/miralandlabs/x402/blob/master/SUBSCRIPTION_PATTERN.md) | Wire contract, errors, checklists |
| [README.md](../README.md) | Auth API + deploy |
| [x402-subscription-starter](https://github.com/miralandlabs/x402-subscription-starter) | Forkable seller |
| [x402-subscription-client](https://github.com/miralandlabs/x402-subscription-client) | Buyer SDK |
