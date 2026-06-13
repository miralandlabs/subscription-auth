# AGENTS.md — subscription-auth

For AI agents. **Simple is Best, yet Elegant.**

Hosted subscription JWT signing oracle (RS256, JWKS, wallet-signed admin). **Not x402-specific** — general-purpose; x402 sellers integrate via `@pr402/subscription-seller` Tier B.

## Topology

- `src/bin/auth_api.rs` — Vercel router (`vercel-rust@4.0.8`)
- `src/api/` — HTTP handlers
- `src/db.rs` — **all** Postgres access (`AuthDb`)
- `src/challenge_auth.rs` — wallet challenge (domain `x402 subscription auth v1`)
- `migrations/init.sql` — canonical schema; mirror changes in `001_*.sql`

## DB rules (non-negotiable — solrisk pattern)

Every SQL call through `AuthDb` only:

1. `timeout(20s, pool.get())`
2. `BEGIN` → `SET LOCAL statement_timeout = '25s'` → `DEALLOCATE ALL` (hard fail) → query → `COMMIT`
3. Wrap every wire step in tokio `timeout`
4. `RecyclingMethod::Clean`, `max_size: 5`
5. **Never** raw SQL in handlers
6. Migrations: manual `psql` only — never auto-run on deploy
7. Any `NNN_*.sql` change **must** duplicate in `init.sql`

## Verify before done

```bash
cargo fmt --all -- --check
cargo clippy --bin auth_api -- -D warnings
cargo test
```
