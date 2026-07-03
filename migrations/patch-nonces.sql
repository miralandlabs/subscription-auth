-- Idempotent patch: challenge nonces (required for /v1/services/{wallet}/challenge)
-- NOTE: This table is already included in migrations/init.sql and migrations/001_initial.sql.
-- Only apply this file if you ran an older schema that pre-dates the nonces table.
-- Run: psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f migrations/patch-nonces.sql

CREATE TABLE IF NOT EXISTS subscription_auth_nonces (
    wallet                TEXT NOT NULL,
    nonce                 TEXT NOT NULL,
    expires_at            TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (wallet, nonce)
);

CREATE INDEX IF NOT EXISTS idx_subscription_auth_nonces_expires
    ON subscription_auth_nonces (expires_at);
