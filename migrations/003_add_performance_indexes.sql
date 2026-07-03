-- Add performance indexes for pagination and filtering
-- Apply: psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f migrations/003_add_performance_indexes.sql

-- Index for merchant wallet queries with active status filter
-- Supports: SELECT * FROM services WHERE merchant_wallet = ? AND status = 'active'
CREATE INDEX IF NOT EXISTS idx_subscription_auth_services_wallet_status
    ON subscription_auth_services (merchant_wallet, status)
    WHERE status = 'active';

-- Index for payer lookups (useful for buyer-side token queries)
-- Supports: SELECT * FROM tokens WHERE payer = ? ORDER BY issued_at DESC
CREATE INDEX IF NOT EXISTS idx_subscription_auth_tokens_payer_issued
    ON subscription_auth_tokens (payer, issued_at DESC);

-- Composite index for efficient merchant subscription pagination
-- This supports the JOIN query in list_subscriptions:
-- SELECT t.* FROM tokens t JOIN services s ON t.service_id = s.service_id
-- WHERE s.merchant_wallet = ? ORDER BY t.issued_at DESC
-- Note: The existing idx_subscription_auth_tokens_service_issued already handles part of this,
-- but for very large datasets, consider a denormalized merchant_wallet column on tokens table.

COMMENT ON INDEX idx_subscription_auth_services_wallet_status IS 
'Optimizes active service lookups by merchant wallet';

COMMENT ON INDEX idx_subscription_auth_tokens_payer_issued IS 
'Optimizes payer token history queries with chronological ordering';
