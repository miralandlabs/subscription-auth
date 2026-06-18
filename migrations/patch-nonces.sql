-- Idempotent patch: challenge nonces (required for /v1/services/{wallet}/challenge)
-- Run: psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f migrations/patch-nonces.sql

CREATE TABLE IF NOT EXISTS subscription_auth_nonces (
    wallet                TEXT NOT NULL,
    nonce                 TEXT NOT NULL,
    expires_at            TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (wallet, nonce)
);

CREATE INDEX IF NOT EXISTS idx_subscription_auth_nonces_expires
    ON subscription_auth_nonces (expires_at);
