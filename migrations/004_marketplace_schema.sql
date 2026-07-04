-- Upgrade path for databases created before marketplace catalog (Phase 1).
-- Fresh installs: use init.sql only — it already includes category, tags, and indexes.
-- Apply: psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f migrations/004_marketplace_schema.sql

ALTER TABLE subscription_auth_services
    ADD COLUMN IF NOT EXISTS category TEXT,
    ADD COLUMN IF NOT EXISTS tags JSONB NOT NULL DEFAULT '[]'::jsonb;

CREATE INDEX IF NOT EXISTS idx_subscription_auth_services_category
    ON subscription_auth_services (category)
    WHERE status = 'active';

CREATE INDEX IF NOT EXISTS idx_subscription_auth_services_tags
    ON subscription_auth_services USING GIN (tags)
    WHERE status = 'active';

CREATE INDEX IF NOT EXISTS idx_subscription_auth_services_marketplace_list
    ON subscription_auth_services (updated_at DESC, service_id)
    WHERE status = 'active';
