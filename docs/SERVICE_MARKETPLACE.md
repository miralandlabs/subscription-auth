# Subscription Catalog Design

**Status:** Phase 1 implemented (read APIs + pr402-registry Subscriptions tab)  
**Author:** AI Agent  
**Date:** 2026-07-04

## Problem

Registered services exist in the database, but discovery is manual. Sellers register via CLI script, buyers get URLs out-of-band. No browsing, no comparison, no ecosystem visibility.

## Vision

Extend subscription-auth with a **read-only subscription catalog** surfaced in **[pr402-registry](https://registry.pr402.org) → Subscriptions tab** (no standalone marketplace SPA):

1. **Sellers register once** (Tier B auth) → optional `tier_bundles` metadata → product appears in Subscriptions tab
2. **Buyers browse** → filter by category, tags, pricing summary
3. **Buyers subscribe** on the **seller** `POST /api/v1/subscribe` (402 + pr402) — not via auth in Phase 1
4. **Network effects** → more subscription products = more buyers

## Relationship to pr402-registry Resources tab

Two **parallel** discovery indexes — not parent/child:

| Tab | Unit | Data source | Audience |
|-----|------|-------------|----------|
| **Resources** | One payable HTTP URL | pr402 `payable_resources` | Agents (`GET /resources`, 402 probe) |
| **Subscriptions** | One time-window product | subscription-auth `tier_bundles` | Humans browsing tiers/SLA |

- Pay-per-call APIs appear in **Resources only**.
- Subscription sellers may optionally enroll `/subscribe?tier=…` URLs in pr402 **Resources** for agent discovery — separate from the **Subscriptions** catalog row.
- Authoritative subscribe pricing always comes from **live HTTP 402** on the seller subscribe endpoint.

## Current State

### Storage (`subscription_auth_services`)

```sql
service_id            TEXT PRIMARY KEY
merchant_wallet       TEXT NOT NULL
service_url           TEXT NOT NULL
resources_allowlist   JSONB NOT NULL DEFAULT '[]'::jsonb
tier_bundles          JSONB                              -- exists, unused
status                TEXT ('active', 'retired')
created_at            TIMESTAMPTZ
updated_at            TIMESTAMPTZ
```

**Issues (resolved in Phase 1 unless noted):**
- ~~`tier_bundles` undefined schema~~ → validated on register/update (`src/tier_bundles.rs`)
- ~~No category/tags index~~ → `category` + `tags` in `migrations/init.sql` (upgrade: `004_marketplace_schema.sql`)
- ~~No discoverability~~ → `GET /v1/marketplace/subscriptions`
- ~~`/v1/info/{service_id}` excludes tier_bundles~~ → now included

### Endpoints

**Write:**
- `POST /v1/services/{wallet}/register` — creates service (optional `tier_bundles`)
- `POST /v1/services/{wallet}/update` — updates allowlist/tier_bundles

**Read (catalog — Phase 1):**
- `GET /v1/marketplace/subscriptions` — list active products (filter, paginate)
- `GET /v1/marketplace/subscriptions/{service_id}` — full detail
- `GET /v1/info/{service_id}` — seller self-check (includes `tier_bundles`)

## Design Goals

**Simple:** Sellers fill 5 fields, not 50  
**Concise:** One endpoint for discovery, one for detail  
**Clear:** Pricing visible without diving into docs  
**Clean:** Schema enforces required fields, validates structure

## Proposed Schema

### `tier_bundles` Structure

```jsonb
{
  "display": {
    "name": "TikTok Video API",
    "tagline": "Fetch video metadata and comments",
    "description": "Production-grade TikTok data API...",
    "icon_url": "https://cdn.example.com/tiktok-icon.png",
    "category": "social-media",
    "tags": ["tiktok", "video", "metadata"]
  },
  "tiers": [
    {
      "id": "free",
      "name": "Free",
      "price_usdc_per_month": 0,
      "price_usdc_per_request": 0,
      "rate_limit": "10/minute",
      "features": ["Basic video info", "1k requests/month"]
    },
    {
      "id": "pro",
      "name": "Pro",
      "price_usdc_per_month": 10.00,
      "price_usdc_per_request": 0.001,
      "rate_limit": "1000/minute",
      "features": ["Video + comments", "100k req/month", "Priority support"]
    }
  ],
  "compliance": {
    "privacy_policy_url": "https://api.example.com/privacy",
    "terms_url": "https://api.example.com/terms",
    "data_retention_days": 30
  },
  "sla": {
    "uptime_guarantee": "99.9%",
    "support_email": "support@example.com",
    "status_page_url": "https://status.example.com"
  }
}
```

**Why this structure:**
- **Nested, not flat** — clear grouping (display vs pricing vs compliance)
- **Flexible tiers** — supports freemium, pay-per-use, subscription
- **Buyer transparency** — SLA + compliance upfront
- **Optional richness** — start with `display.name`, add `sla` later

### New Table: `service_stats` (future)

```sql
CREATE TABLE service_stats (
    service_id         TEXT PRIMARY KEY REFERENCES subscription_auth_services,
    total_subscribers  INT DEFAULT 0,
    active_tokens      INT DEFAULT 0,
    uptime_30d         NUMERIC(5,2),
    avg_response_ms    INT,
    last_health_check  TIMESTAMPTZ,
    updated_at         TIMESTAMPTZ DEFAULT NOW()
);
```

**Enables:**
- Social proof (subscriber count)
- Quality signals (uptime, latency)
- Trust indicators (health check freshness)

## Catalog Endpoints (Phase 1 — implemented)

### 1. `GET /v1/marketplace/subscriptions`

List active subscription products with filtering and pagination.

**Query params:**
```
?category=social-media
&tags=tiktok,video          # AND filter (must contain all tags)
&q=tiktok                   # search service_id, display.name, tagline
&limit=20
&cursor=...                 # keyset pagination (base64 updated_at|service_id)
```

**Response:**
```json
{
  "subscriptions": [
    {
      "service_id": "api.tiktok.example.com",
      "display": { "name": "TikTok Video API", "category": "social-media", "tags": ["tiktok"] },
      "pricing_summary": {
        "configured": true,
        "starting_at_usdc": 0,
        "has_free_tier": true,
        "billing_models": ["subscription"]
      },
      "catalog_configured": true
    }
  ],
  "pagination": { "next_cursor": "...", "has_more": false, "total": 1 },
  "notice": "Advisory catalog metadata..."
}
```

Services with `tier_bundles=null` appear with `catalog_configured: false`.

### 2. `GET /v1/marketplace/subscriptions/{service_id}`

Full product detail: display, tiers, compliance, SLA, merchant wallet, service URL.

### 3. `POST /v1/marketplace/subscriptions/{service_id}/subscribe` (Phase 3 — future)

One-click subscribe from registry UI → auth → pr402 settle → seller `/subscribe`. **Deferred.**

## Categories & Tags

### Predefined Categories

```
"social-media"      # TikTok, Twitter, Instagram APIs
"blockchain"        # Solana RPC, indexers, DeFi data
"ai-ml"             # LLM inference, embeddings, OCR
"finance"           # Pricing feeds, forex, market data
"developer-tools"   # CI/CD, code analysis, formatters
"content"           # News, articles, media assets
"communication"     # Email, SMS, push notifications
"other"
```

**Validation:** Enum in DB, reject unknown categories on register.

### Freeform Tags

Array of strings, max 10 tags per service, lowercase normalized.

**Examples:** `["tiktok", "video", "metadata", "comments", "realtime"]`

**Searchable:** Full-text search or `tags @> '["tiktok"]'::jsonb` index.

## Migration Path

### Phase 1: Schema + Read (v0.2) — **done**

1. `tier_bundles` validation on register/update
2. `GET /v1/marketplace/subscriptions` (list)
3. `GET /v1/marketplace/subscriptions/{service_id}` (detail)
4. `/v1/info/{service_id}` includes `tier_bundles`
5. `category` and `tags` columns — in `init.sql`; existing DBs use `004_marketplace_schema.sql`
6. **pr402-registry** Subscriptions tab (read-only proxy to auth host)

**Backward compat:** `tier_bundles=null` → `catalog_configured: false` in UI.

### Phase 2: Stats + Quality (v0.3) — future

1. `service_stats` table
2. Background job: periodic health checks → update uptime
3. Increment `total_subscribers` on token issue
4. Add `?sort=uptime_desc&min_uptime=99` filters

### Phase 3: Integrated Subscribe (v0.4) — future

1. `POST /v1/marketplace/subscriptions/{service_id}/subscribe`
2. Wallet connect in pr402-registry detail drawer (optional)
3. Wallet adapter (Phantom, Backpack, Solflare)

## Seller workflow (recommended)

1. **pr402 activate** SplitVault (required for payments)
2. **Optional:** enroll subscribe URLs in pr402 **Resources** tab (agent discovery)
3. **Tier B:** register in subscription-auth (`register-service.mjs`)
4. **Optional:** pass `--metadata metadata.json` with `tier_bundles` for **Subscriptions** tab

### Register with catalog metadata

```bash
node scripts/register-service.mjs \
  --keypair seller.json \
  --service-id api.example.com \
  --service-url https://api.example.com \
  --allowlist "/api/v1/data" \
  --metadata metadata.json
```

**`metadata.json`:**
```json
{
  "display": {
    "name": "My API",
    "tagline": "Fast and reliable",
    "category": "developer-tools",
    "tags": ["api", "data"]
  },
  "tiers": [
    { "id": "free", "name": "Free", "price_usdc_per_month": 0, "rate_limit": "10/min" }
  ]
}
```

Service appears in **registry.pr402.org → Subscriptions** after deploy + metadata.

## Buyer Experience

### Discovery Flow (Phase 1)

1. Visit **https://registry.pr402.org** → **Subscriptions** tab
2. Filter: `category=social-media`, tags, search
3. Open product card → tiers, SLA, compliance
4. Follow link to seller `GET /api/v1/subscribe/info` or subscribe URL
5. Pay via seller `POST /api/v1/subscribe` (402 + pr402) → JWT → data routes

**No email, no password.** Subscribe payment stays on the seller, not subscription-auth.

## Advanced Features (Future)

### 1. Service Reviews & Ratings

```sql
CREATE TABLE service_reviews (
    id              UUID PRIMARY KEY,
    service_id      TEXT REFERENCES subscription_auth_services,
    reviewer_wallet TEXT NOT NULL,
    rating          INT CHECK (rating BETWEEN 1 AND 5),
    comment         TEXT,
    created_at      TIMESTAMPTZ DEFAULT NOW()
);
```

**Constraint:** Only active token holders can review (via JWT introspect).

### 2. Referral Program

```json
{
  "referral": {
    "code": "TIKTOK10",
    "discount_percent": 10,
    "referrer_reward_percent": 5
  }
}
```

Seller earns 5% when referred buyer subscribes. Stored in `tier_bundles.referral`.

### 3. Usage-Based Pricing

```json
{
  "tiers": [
    {
      "id": "payg",
      "name": "Pay As You Go",
      "price_usdc_per_request": 0.0001,
      "included_requests": 0,
      "overage_price_usdc": 0.0001
    }
  ]
}
```

Auth tracks request count, auto-issues invoice when threshold hit.

### 4. Service Bundles

```json
{
  "bundle": {
    "id": "social-suite",
    "name": "Social Media Suite",
    "services": ["api.tiktok.example.com", "api.twitter.example.com"],
    "price_usdc_per_month": 25.00,
    "discount_percent": 20
  }
}
```

Multi-service subscription, single JWT with `aud: ["svc1", "svc2"]`.

### 5. API Analytics Dashboard

Seller dashboard at `/v1/marketplace/dashboard/{merchant_wallet}`:
- Subscriber growth chart
- Revenue (USDC) over time
- Top resources by request count
- Uptime history
- Active tokens by tier

### 6. Compliance Badges

Auto-verify compliance claims:
- ✅ **Privacy Policy** (URL returns 200)
- ✅ **SOC 2** (upload cert, expires 2027-01-01)
- ✅ **GDPR Ready** (seller attestation)

Badges display on service card, build trust.

## Security Considerations

### 1. Spam Prevention

**Problem:** Free registration → spam services flood marketplace.

**Mitigations:**
- Require SOL deposit (0.1 SOL) on first register, refunded on retire
- Rate limit: 5 services per merchant wallet
- Admin moderation queue (flag inappropriate content)

### 2. Malicious Metadata

**Problem:** XSS via `display.name` or `icon_url`.

**Mitigations:**
- Validate JSON Schema on register (reject `<script>`)
- Sanitize HTML in marketplace UI
- `icon_url` must be HTTPS, validated domain whitelist (or IPFS CID)

### 3. Fake Stats

**Problem:** Seller inflates `total_subscribers` to game ranking.

**Mitigations:**
- Stats computed by auth (not seller-provided)
- `total_subscribers` = `COUNT(*)` from `subscription_auth_tokens`
- Uptime from auth health checks (seller can't fake)

## Implementation Checklist

### Schema (Phase 1)

- [x] `tier_bundles` validator (`src/tier_bundles.rs`)
- [x] `category TEXT` + `tags JSONB` columns
- [ ] `service_stats` table (Phase 2)
- [x] `category TEXT` + `tags JSONB` in `init.sql` (upgrade: `004_marketplace_schema.sql`)

### API (Phase 1)

- [x] `GET /v1/marketplace/subscriptions`
- [x] `GET /v1/marketplace/subscriptions/{service_id}`
- [x] Validate `tier_bundles` on register/update
- [x] `GET /v1/info/{service_id}` includes `tier_bundles`
- [x] `src/api/marketplace.rs`

### Registry UI (Phase 1)

- [x] pr402-registry **Subscriptions** tab (`SubscriptionsTab.tsx`)
- [x] Proxy `/api/v1/marketplace/*` → auth.ipay.sh / preview.auth.ipay.sh

### DB (Phase 2+)

- [ ] `service_stats` + health job
- [ ] `?sort=uptime_desc` filters

### Scripts (future)

- [ ] `register-service.mjs --metadata metadata.json`
- [ ] `scripts/list-marketplace.mjs`

### Testing

- [x] Unit: tier_bundles validation
- [ ] Integration: register → list → detail

## Success Metrics

**Month 1:**
- 10 services registered with full metadata
- 100 marketplace page views
- 5 wallet-based subscriptions via marketplace

**Month 3:**
- 50 services across 5 categories
- 1,000 active subscribers (across all services)
- Avg time-to-subscribe < 60 seconds

**Month 6:**
- 200 services
- 10,000 subscribers
- First service earning $1k/month via marketplace
- Ecosystem flywheel: sellers invite sellers

## References

- [API Marketplace Screenshot](context) — initial inspiration
- [Stripe Apps Marketplace](https://marketplace.stripe.com) — app listing pattern
- [RapidAPI](https://rapidapi.com) — API discovery model
- [OpenSea](https://opensea.io) — NFT marketplace (wallet-based subscribe parallel)

## Next Steps

1. Deploy subscription-auth (`init.sql` on fresh DB; `004_marketplace_schema.sql` on existing)
2. Deploy pr402-registry with Subscriptions tab
3. Add `--metadata` to `register-service.mjs`
4. Phase 2: stats + quality signals
5. Phase 3: integrated wallet subscribe (optional)

---

**Simple. Concise. Clear. Clean.**  
Subscription catalog + pr402-registry Subscriptions tab — discovery for time-window products alongside Resources.
