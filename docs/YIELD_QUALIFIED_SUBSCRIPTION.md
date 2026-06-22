# Yield-qualified subscription (not a pr402 rail)

**Marketing pattern:** user holds ≥ $X in a yield-bearing stablecoin or RWA (e.g. USDY on Solana) → seller grants or extends subscription access without a new **`exact`** payment each window.

**This is not a third pr402 scheme.** pr402 stays **`exact`** + **`sla-escrow`** only. Yield qualification lives at the **seller + optional middleware** layer.

**Related:** [SUBSCRIPTION_PATTERN.md](https://github.com/miralandlabs/x402/blob/master/SUBSCRIPTION_PATTERN.md) (wire contract) · [SUBSCRIPTION_AUTH_FOR_SELLERS.md](SUBSCRIPTION_AUTH_FOR_SELLERS.md) (Tier A/B JWT)

---

## Four layers (keep them separate)

| Layer | What | Who moves money | pr402 involved? |
|-------|------|-----------------|-------------------|
| **A. Core product** | API, tool, data feed | N/A | No |
| **B. Qualification** | Hold ≥ threshold in allowlisted mint(s) | User swaps via Jupiter; funds in **user-owned** ATA | **No settle** |
| **C. Yield collection** | Sweep appreciation via SPL `approve` delegate | Seller/middleware pulls surplus periodically | **No** — not UniversalSettle |
| **D. Subscription window** | JWT + Bearer on data routes | After **`exact` pay** and/or after qualification | JWT only on data routes |

```text
x402 layer:     HTTP 402 + proof of entitlement
pr402 layer:    ONLY when proof = on-chain payment (exact | sla-escrow)
Seller layer:   WHAT counts as entitlement (paid | JWT | balance | dual)
Product layer:  WHY the customer comes
Yield layer:    HOW you monetize without touching principal (optional middleware)
```

---

## pr402 rails: no third category

| Rail | Use | On-chain |
|------|-----|----------|
| **`exact`** | Instant payment; subscription **purchase** on `/subscribe` | UniversalSettle SplitVault |
| **`sla-escrow`** | Conditional delivery (tokens, files, jobs) | SLA-Escrow program |
| **Yield hold** | **Not a rail** — seller entitlement policy | User wallet + optional delegate |

Do **not** add `build-hold-qualification-tx`, swap builders, or hold-oracle APIs to pr402. Jupiter bundles and baseline tracking belong in **seller app** or a **standalone yield-gateway** repo.

---

## Two paths to `/subscribe`

| Path | Trigger | pr402 `verify`/`settle` | Facilitator protocol fee |
|------|---------|-------------------------|--------------------------|
| **Pay tier** (today) | `PAYMENT-SIGNATURE` + **`exact`** | Yes | Yes |
| **Hold tier** (optional) | Wallet ≥ N USD in allowlisted mint(s) | **No** | **No** |

Hold renew: seller cron re-checks balance → extend JWT; if below threshold → `401 TOKEN_EXPIRED` (same as today).

### Who pays for hold-mode infra?

| Model | pr402 | Who pays |
|-------|-------|----------|
| **A. Seller-only hold** (recommended) | Unchanged | Seller runs RPC/Jupiter |
| **B. Hybrid renew** | **`exact` micro-settle** each window (e.g. $0.05) | Buyer; pr402 earns per renew |
| **C. Seller SaaS** | Optional paid qualification API (off pr402) | Seller → Miraland B2B |
| **D. Yield middleware** | **`exact`** when agent pays bills; middleware takes yield % | Both |
| **E. Discovery listing** | Indirect | Merchant listing fees |

**Design rule:** pr402 must not ship free settlement-adjacent helpers (swap builder, sponsor, hold oracle) that bypass the fee rail.

### Funnel framing (keeps pr402 whole)

1. **Acquire** — “Hold $300 USDY → unlock trial” (seller/middleware; no pr402).
2. **Convert** — paid tier or per-call still uses **`exact`** (pr402 earns).
3. **Renew** — periodic **`exact`** renew **or** seller-funded hold check (seller accepts $0 facilitator fee on that path).

---

## Solana notes (USDY example)

- **USDY on Solana:** accumulating price (not 1:1 with USDC); **6 decimals**; mint `A1KLoBrKBde8Ty9qtNQUtq3C2ortoC3u7twggz7sEto6`.
- **No native rUSDY rebase on Solana** — track **USD value** = `(token_amount × current_price) − baseline_usd`.
- **Non-custodial path:** user-owned ATA + `spl-token approve` delegate — **not** deposits to operator business wallet.

Exploration spike (read-only): [scripts/check-hold-qualification.mjs](../scripts/check-hold-qualification.mjs) — zero npm deps; Node 18+ only.

---

## Compliance & custody (architecture only — not legal advice)

pr402 **`exact`** and **`sla-escrow`** are **designed non-custodial**: buyer signs; USDC flows to **program PDAs** (SplitVault / Escrow), not facilitator operating wallets. See [ARCHITECTURE_OVERVIEW.md](https://github.com/miralandlabs/x402/blob/master/ARCHITECTURE_OVERVIEW.md).

| | **`exact`** | **`sla-escrow`** | **Yield hold / deposit** |
|---|-------------|------------------|--------------------------|
| Operator pooled wallet | No | No | **Often yes** |
| Timed hold | Seconds | Hours/days | Indefinite |
| Relative scrutiny | Lower | **Higher** | **Highest** |

Yield qualification (swap → hold → delegate sweep) is **more custody/transmission-shaped** than pr402 rails — keep it **off pr402**.

**RWA primary issuance** ([x402-buy-rwa-token](https://github.com/miralandlabs/x402-buy-rwa-token)) is a **separate regulated stack** (KYC portal, transfer hook) — not the same as secondary-market USDY promos.

Consult qualified counsel before production compliance claims. Prioritize **`sla-escrow`** + keeper/oracle review before broad marketing.

---

## Where to build (if you experiment)

| Component | Repo / layer |
|-----------|----------------|
| Hold check + JWT extend | [x402-subscription-starter](https://github.com/miralandlabs/x402-subscription-starter) seller |
| RS256 issue after qualify | [subscription-auth](../README.md) Tier B — [SUBSCRIPTION_AUTH_FOR_SELLERS.md](SUBSCRIPTION_AUTH_FOR_SELLERS.md) |
| Jupiter + delegate + sweep | Standalone yield-gateway (future) |
| Payment + protocol fee | pr402 **`exact`** only |

---

## References

| Doc | Use |
|-----|-----|
| [SUBSCRIPTION_PATTERN.md](https://github.com/miralandlabs/x402/blob/master/SUBSCRIPTION_PATTERN.md) | Wire contract + optional hold-tier summary |
| [SUBSCRIPTION_AUTH_FOR_SELLERS.md](SUBSCRIPTION_AUTH_FOR_SELLERS.md) | Tier A/B JWT setup |
| [ARCHITECTURE_OVERVIEW.md](https://github.com/miralandlabs/x402/blob/master/ARCHITECTURE_OVERVIEW.md) | pr402 rails + non-custodial invariants |
