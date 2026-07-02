# Code Review Report - subscription-auth

**Date:** July 2, 2026  
**Reviewer:** AI Assistant  
**Scope:** Complete codebase review focusing on security, reliability, and best practices

---

## Executive Summary

The codebase is **well-structured and security-conscious**. It follows the "solrisk hardened transaction pattern" for database operations and implements wallet-based authentication with HMAC-bound challenges. Overall code quality is high with good separation of concerns.

### Critical Issues: 0
### High Priority Issues: 2
### Medium Priority Issues: 4
### Low Priority/Improvements: 6

---

## Critical Issues

None found.

---

## High Priority Issues

### H1. Missing CORS Preflight Cache Control Headers
**Location:** `src/http_util.rs:cors_options()`  
**Severity:** High  
**Impact:** Excessive preflight requests from browsers

**Issue:**
```rust
pub fn cors_options() -> Response<Body> {
    let builder = Response::builder()
        .status(VercelStatusCode::NO_CONTENT)
        .header("Access-Control-Max-Age", "86400");
    add_cors_headers(builder).body(Body::Empty).unwrap()
}
```

The `not_found()` handler in `auth_api.rs` doesn't include CORS headers, which means failed preflight OPTIONS requests to invalid routes won't include CORS headers.

**Recommendation:**
Add CORS headers to the `not_found()` handler:
```rust
fn not_found() -> Response<Body> {
    let builder = Response::builder()
        .status(StatusCode::NOT_FOUND);
    crate::http_util::add_cors_headers(builder)
        .body(Body::Text("Not found".into()))
        .unwrap()
}
```

### H2. Nonce Table Cleanup Not Implemented
**Location:** Database schema and operations  
**Severity:** High  
**Impact:** Unbounded table growth, potential performance degradation

**Issue:**
The `subscription_auth_nonces` table has an index on `expires_at` but there's no cleanup job to delete expired nonces. Over time, this table will grow unbounded.

**Recommendation:**
Add a periodic cleanup job or use PostgreSQL partitioning with automatic partition dropping. Quick fix: add a migration for TTL-based cleanup:
```sql
-- Option 1: Add to migrations
DELETE FROM subscription_auth_nonces WHERE expires_at < NOW() - INTERVAL '1 hour';

-- Option 2: Create a periodic cleanup function
CREATE OR REPLACE FUNCTION cleanup_expired_nonces()
RETURNS void AS $$
BEGIN
    DELETE FROM subscription_auth_nonces WHERE expires_at < NOW() - INTERVAL '1 hour';
END;
$$ LANGUAGE plpgsql;
```

---

## Medium Priority Issues

### M1. Unvalidated `recovery_wallet` Field
**Location:** `src/api/mod.rs:RegisterBody`  
**Severity:** Medium  
**Impact:** Data integrity, potential confusion

**Issue:**
```rust
pub struct RegisterBody {
    // ...
    pub recovery_wallet: Option<String>,  // Accepted but never validated or stored
}
```

The `recovery_wallet` field is accepted in the `RegisterBody` struct but is never validated, stored, or used. This creates a misleading API surface.

**Recommendation:**
Either implement recovery wallet functionality or remove the field from the struct.

### M2. Hardcoded Rate Limits
**Location:** `src/api/mod.rs:handle_challenge()`  
**Severity:** Medium  
**Impact:** Inflexible rate limiting

**Issue:**
```rust
const MAX_OUTSTANDING_NONCES: i64 = 10;
```

The rate limit of 10 outstanding nonces per wallet is hardcoded. For production services with high traffic, this may be too restrictive.

**Recommendation:**
Make this configurable via environment variable:
```rust
let max_nonces = std::env::var("SUBSCRIPTION_AUTH_MAX_NONCES_PER_WALLET")
    .ok()
    .and_then(|s| s.parse().ok())
    .unwrap_or(10);
```

### M3. Missing Index for Efficient Pagination
**Location:** `migrations/init.sql`  
**Severity:** Medium  
**Impact:** Poor query performance for large datasets

**Issue:**
The `list_subscriptions` query filters by `merchant_wallet` via JOIN and orders by `issued_at`, but there's no composite index to support this efficiently.

**Current:**
```sql
CREATE INDEX IF NOT EXISTS idx_subscription_auth_tokens_service_issued
    ON subscription_auth_tokens (service_id, issued_at DESC);
```

**Recommendation:**
Add an index for the merchant wallet pagination query:
```sql
CREATE INDEX IF NOT EXISTS idx_subscription_auth_tokens_merchant_issued
    ON subscription_auth_tokens (service_id, issued_at DESC);
    
-- Also consider adding for the JOIN optimization:
CREATE INDEX IF NOT EXISTS idx_subscription_auth_services_wallet_active
    ON subscription_auth_services (merchant_wallet, status)
    WHERE status = 'active';
```

### M4. No Request Body Size Limit
**Location:** `src/bin/auth_api.rs:body_to_string()`  
**Severity:** Medium  
**Impact:** Potential DoS via large payloads

**Issue:**
```rust
async fn body_to_string(body: Body) -> Result<String, String> {
    match body {
        Body::Text(s) => Ok(s),
        Body::Binary(b) => String::from_utf8(b)...
```

There's no size limit on request bodies, allowing attackers to send extremely large payloads.

**Recommendation:**
Add a reasonable size limit (e.g., 1MB) and reject larger requests:
```rust
async fn body_to_string(body: Body) -> Result<String, String> {
    const MAX_BODY_SIZE: usize = 1_048_576; // 1MB
    match body {
        Body::Text(s) => {
            if s.len() > MAX_BODY_SIZE {
                return Err("request body too large".to_string());
            }
            Ok(s)
        }
        Body::Binary(b) => {
            if b.len() > MAX_BODY_SIZE {
                return Err("request body too large".to_string());
            }
            String::from_utf8(b).map_err(|_| "request body is not valid UTF-8".to_string())
        }
        Body::Empty => Ok(String::new()),
    }
}
```

---

## Low Priority Issues / Improvements

### L1. Error Messages Leak Internal State
**Location:** Multiple handlers  
**Severity:** Low  
**Impact:** Information disclosure

**Issue:**
Internal errors are sometimes passed directly to clients:
```rust
.map_err(|e| Error::Internal(e.to_string()))?
```

This can leak database error messages, table names, or other internal details.

**Recommendation:**
Log detailed errors server-side but return generic messages to clients:
```rust
.map_err(|e| {
    tracing::error!("Database error: {}", e);
    Error::Internal("database operation failed".into())
})?
```

### L2. Missing Request ID / Tracing Context
**Location:** `src/bin/auth_api.rs`  
**Severity:** Low  
**Impact:** Difficult to trace requests across logs

**Issue:**
There's no request ID or correlation ID for tracking requests through the system.

**Recommendation:**
Add a request ID to each request and include it in all log statements:
```rust
use uuid::Uuid;

let request_id = Uuid::new_v4();
tracing::info!(request_id = %request_id, method = %method, path = %path, "handling request");
```

### L3. Unused `parameters` Table
**Location:** `migrations/init.sql`  
**Severity:** Low  
**Impact:** Unused database resources

**Issue:**
The `parameters` table is defined in the schema but never referenced in the code.

**Recommendation:**
Either implement feature flags/parameters functionality or remove the table.

### L4. Missing Metrics/Observability
**Location:** Entire codebase  
**Severity:** Low  
**Impact:** Limited production visibility

**Issue:**
No metrics collection for:
- Request counts by endpoint
- Request duration
- Error rates
- Database connection pool stats
- Token issuance rates

**Recommendation:**
Add metrics using a library like `metrics` or integrate with your monitoring system.

### L5. No Health Check for JWKS Endpoint
**Location:** `src/api/mod.rs:handle_health()`  
**Severity:** Low  
**Impact:** Incomplete health check

**Issue:**
The health check only pings the database but doesn't verify RSA key loading or JWKS generation.

**Recommendation:**
```rust
pub async fn handle_health(state: Arc<AppState>) -> Response<Body> {
    let mut body = json!({
        "status": "ok",
        "service": "subscription-auth",
        "db": if state.db.is_some() { "configured" } else { "missing" }
    });
    
    if let Some(db) = &state.db {
        match db.ping().await {
            Ok(()) => body["db_ping"] = json!("ok"),
            Err(e) => body["db_ping"] = json!(format!("error: {e}")),
        }
    }
    
    // Add JWKS check
    match state.jwks_document().await {
        Ok(_) => body["jwks"] = json!("ok"),
        Err(e) => body["jwks"] = json!(format!("error: {e}")),
    }
    
    json_response(200, &body)
}
```

### L6. Inconsistent Error Message Format
**Location:** `src/challenge_auth.rs`, `src/service_id.rs`  
**Severity:** Low  
**Impact:** Inconsistent user experience

**Issue:**
Some error messages use sentence case, others don't. Some have periods, others don't.

**Example:**
```rust
"invalid wallet pubkey"
"HMAC mismatch"
"service_id is required"
"service_id too long"
```

**Recommendation:**
Standardize error messages: lowercase, no periods for single phrases, capitalized sentences with periods for longer explanations.

---

## Security Strengths

✅ **Excellent:**
1. **HMAC-bound challenges** prevent tampering and replay attacks
2. **Nonce consumption** prevents double-use of challenges  
3. **Constant-time HMAC comparison** prevents timing attacks
4. **Wallet signature verification** ensures authenticity
5. **Action-specific challenges** prevent privilege escalation (e.g., retire challenge can't be used for update)
6. **Resources allowlist validation** prevents unauthorized token issuance
7. **TLS certificate verification** enabled by default (MITM protection)
8. **SQL injection** protected via parameterized queries
9. **Connection pool hardening** with timeouts and transaction guardrails

---

## Performance Considerations

✅ **Good:**
1. Connection pooling with appropriate timeouts
2. Keyset pagination for subscriptions
3. Strategic database indexes

⚠️ **Needs Attention:**
1. Missing composite indexes for some query patterns (see M3)
2. No query result caching (consider for JWKS endpoint)
3. Nonce table will grow unbounded without cleanup (see H2)

---

## Code Quality

✅ **Excellent:**
1. Clear separation of concerns (API, DB, auth, config)
2. Comprehensive error handling with typed errors
3. Well-documented solrisk transaction pattern
4. Unit tests for critical path (challenge auth, validation)
5. Sensible constants and timeouts

⚠️ **Minor Issues:**
1. Some duplicated error mapping logic
2. Could benefit from more integration tests
3. Missing documentation comments on some public functions

---

## Recommendations Priority

**Immediate (Before Production):**
1. Fix H1: Add CORS to not_found handler
2. Fix H2: Implement nonce cleanup
3. Fix M4: Add request body size limits

**Short Term (Next Sprint):**
1. Fix M1: Handle or remove recovery_wallet
2. Fix M2: Make rate limits configurable  
3. Fix M3: Add missing indexes
4. Fix L1: Improve error message handling

**Medium Term:**
1. Add request tracing (L2)
2. Add metrics/observability (L4)
3. Enhance health checks (L5)

---

## Test Coverage

**Current:** 9 unit tests covering:
- Challenge auth (register, issue, HMAC tampering)
- Service ID validation (DNS-style, wallet-qualified, flat rejection)
- Resources allowlist validation
- Query parsing

**Gaps:**
- No integration tests for API endpoints
- No tests for database operations
- No tests for JWT issuance and validation
- No tests for pagination
- No tests for error cases in handlers

**Recommendation:**
Add integration tests using a test database:
```rust
#[cfg(test)]
mod integration_tests {
    // Test full register → issue → revoke flow
    // Test pagination edge cases
    // Test error handling
}
```

---

## Conclusion

The codebase demonstrates **strong security practices** and **thoughtful design**. The main concerns are operational (nonce cleanup, observability) rather than fundamental security issues. With the high-priority fixes applied, this system is production-ready for a Tier B auth service.

**Overall Grade: A-**

The deduction is primarily for the missing nonce cleanup mechanism (H2) and operational concerns (metrics, observability). The core security and transaction handling are exemplary.
