# JWKS key rotation

## Overlap window (24h)

1. Generate new RSA keypair; assign new `kid` (e.g. `auth-key-2`)
2. Insert previous public JWK into `subscription_auth_signing_keys` with `retired_at = NOW()`
3. Update Vercel env: `SUBSCRIPTION_AUTH_RSA_PRIVATE_KEY_PEM`, `SUBSCRIPTION_AUTH_KEY_ID`
4. Deploy — JWKS serves **both** keys for 24h while old tokens expire
5. After overlap, remove retired key from JWKS (or leave until `retired_at + 24h` auto-filters)

## Emergency revoke

If private key compromised:

1. Rotate env key immediately (new `kid`)
2. Remove compromised public JWK from JWKS
3. Optionally bulk-revoke by service via `POST /v1/tokens/revoke` per `jti`

Single-wallet handoff without pre-registered recovery wallet does **not** recover from key compromise — see design doc §10.3.
