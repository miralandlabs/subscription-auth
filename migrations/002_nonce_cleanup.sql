-- Add automatic nonce cleanup
-- Apply: psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f migrations/002_nonce_cleanup.sql

-- Function to clean up expired nonces (older than 1 hour)
CREATE OR REPLACE FUNCTION cleanup_expired_nonces()
RETURNS INTEGER AS $$
DECLARE
    deleted_count INTEGER;
BEGIN
    DELETE FROM subscription_auth_nonces 
    WHERE expires_at < NOW() - INTERVAL '1 hour';
    GET DIAGNOSTICS deleted_count = ROW_COUNT;
    RETURN deleted_count;
END;
$$ LANGUAGE plpgsql;

-- Note: For automatic execution, set up a cron job or use pg_cron extension:
-- SELECT cron.schedule('cleanup-nonces', '*/30 * * * *', 'SELECT cleanup_expired_nonces()');
-- Or call this manually/via application scheduler periodically.

COMMENT ON FUNCTION cleanup_expired_nonces() IS 
'Deletes expired nonces (older than 1 hour past expiry). Call periodically to prevent unbounded table growth.';
