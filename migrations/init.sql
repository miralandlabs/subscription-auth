-- subscription-auth canonical schema (greenfield)
-- Apply: psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f migrations/init.sql

CREATE TABLE IF NOT EXISTS subscription_auth_services (
    service_id            TEXT PRIMARY KEY,
    merchant_wallet       TEXT NOT NULL,
    service_url           TEXT NOT NULL,
    resources_allowlist   JSONB NOT NULL DEFAULT '[]'::jsonb,
    tier_bundles          JSONB,
    status                TEXT NOT NULL DEFAULT 'active'
                          CHECK (status IN ('active', 'retired')),
    created_at            TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at            TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_subscription_auth_services_wallet
    ON subscription_auth_services (merchant_wallet);

CREATE INDEX IF NOT EXISTS idx_subscription_auth_services_wallet_status
    ON subscription_auth_services (merchant_wallet, status)
    WHERE status = 'active';

CREATE TABLE IF NOT EXISTS subscription_auth_tokens (
    jti                   UUID PRIMARY KEY,
    service_id            TEXT NOT NULL REFERENCES subscription_auth_services (service_id),
    payer                 TEXT NOT NULL,
    tier                  TEXT NOT NULL,
    resources             JSONB NOT NULL DEFAULT '["*"]'::jsonb,
    issued_at             TIMESTAMPTZ NOT NULL,
    expires_at            TIMESTAMPTZ NOT NULL,
    revoked_at            TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_subscription_auth_tokens_service_issued
    ON subscription_auth_tokens (service_id, issued_at DESC);

CREATE INDEX IF NOT EXISTS idx_subscription_auth_tokens_revoked
    ON subscription_auth_tokens (service_id, revoked_at)
    WHERE revoked_at IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_subscription_auth_tokens_payer_issued
    ON subscription_auth_tokens (payer, issued_at DESC);

CREATE TABLE IF NOT EXISTS subscription_auth_nonces (
    wallet                TEXT NOT NULL,
    nonce                 TEXT NOT NULL,
    expires_at            TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (wallet, nonce)
);

CREATE INDEX IF NOT EXISTS idx_subscription_auth_nonces_expires
    ON subscription_auth_nonces (expires_at);

CREATE TABLE IF NOT EXISTS subscription_auth_signing_keys (
    kid                   TEXT PRIMARY KEY,
    public_jwk            JSONB NOT NULL,
    active_from           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    retired_at            TIMESTAMPTZ
);

CREATE TABLE IF NOT EXISTS parameters (
    id             SERIAL PRIMARY KEY,
    service        TEXT NOT NULL DEFAULT 'subscription-auth',
    endpoint       TEXT NOT NULL DEFAULT '*',
    param_name     TEXT NOT NULL,
    param_value    TEXT NOT NULL,
    inactive       BOOLEAN NOT NULL DEFAULT FALSE,
    effective_from TIMESTAMPTZ,
    expires_at     TIMESTAMPTZ,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS uniq_parameters_service_endpoint_param
    ON parameters (service, endpoint, param_name);
