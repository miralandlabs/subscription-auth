# subscription-auth — Challenge-Response Protocol

This document describes the wire format of the challenge-response mechanism used
by `subscription-auth`. Any language can implement a client from this document —
you do **not** need to read the Rust source.

---

## Overview

All mutating endpoints (register, update, retire, issue, revoke) and the
subscriptions listing endpoint require the caller to:

1. **Obtain a challenge** — `GET /v1/services/{wallet}/challenge?action=...`
2. **Sign the challenge** with an Ed25519 keypair (Solana-compatible)
3. **Submit** the original challenge message + base64 signature in the request body

The server verifies:
- The HMAC over the preimage matches (proves the server issued the message)
- The Ed25519 signature is valid for the wallet in the message
- The nonce has not been consumed before and has not expired (10-minute TTL)
- The declared `action` matches the endpoint being called

---

## Step 1 — Request a Challenge

```
GET /v1/services/{wallet}/challenge?action={action}&{params...}
```

### Action values

| Action     | Endpoint that consumes it              | Required params                                              |
|------------|----------------------------------------|--------------------------------------------------------------|
| `register` | `POST /.../register`                   | `service_id`, `service_url`, `resources_allowlist_json`      |
| `update`   | `POST /.../update`                     | `service_id`, `resources_allowlist_json`                     |
| `retire`   | `POST /.../retire`                     | `service_id`                                                 |
| `issue`    | `POST /v1/tokens/issue`                | `service_id`, `payer`, `tier`, `resources` (JSON array)      |
| `revoke`   | `POST /v1/tokens/revoke`               | `service_id`, `jti`                                          |
| `issue`    | `POST /.../subscriptions` (listing)    | (wallet ownership proof; any `issue` challenge works)        |

### Example request

```
GET /v1/services/Abc...XYZ/challenge?action=register&service_id=api.myapp.com&service_url=https://api.myapp.com&resources_allowlist_json=%5B%22%2Fapi%2Fv1%22%5D
```

### Response

```json
{
  "message": "<full challenge message string>",
  "expires_unix": 1720000600
}
```

---

## Step 2 — The Challenge Message Format

The `message` field is a multi-line UTF-8 string. Sign the **entire string** as-is
(including the trailing newline).

### General structure

```
x402 subscription auth v1

action: {action}
wallet: {wallet}
nonce: {nonce}
timestamp: {iso8601}
expires: {iso8601}
{...bound params (see below)...}

hmac_sha256_hex: {hmac}
```

### Bound params by action

| Action     | Extra lines included in message                                   |
|------------|-------------------------------------------------------------------|
| `register` | `service_id: ...`, `service_url: ...`, `resources_allowlist_json: ...` |
| `update`   | `service_id: ...`, `resources_allowlist_json: ...`                |
| `retire`   | `service_id: ...`                                                 |
| `issue`    | `service_id: ...`, `payer: ...`, `tier: ...`, `resources_json: ...` |
| `revoke`   | `service_id: ...`, `jti: ...`                                     |

> **Important:** The HMAC covers `{preimage}` (every line up to but not including
> the `hmac_sha256_hex:` line, separated by `\n`). The Ed25519 signature covers
> the **entire** `message` string (including the `hmac_sha256_hex:` line).
> Your wallet must sign the full message blob — hardware wallets or UIs will
> display the raw HMAC line to the user, which is expected behaviour.

### Concrete example — `register`

```
x402 subscription auth v1

action: register
wallet: Abc1...XYZ
nonce: 3f7a2b1c
timestamp: 2024-07-01T12:00:00Z
expires: 2024-07-01T12:10:00Z
service_id: api.myapp.com
service_url: https://api.myapp.com
resources_allowlist_json: ["/api/v1/data"]

hmac_sha256_hex: e3b0c44298fc1c149afb...
```

---

## Step 3 — Sign the Message

The signature must be:
- **Algorithm:** Ed25519
- **Input:** the full `message` string, UTF-8 encoded bytes
- **Output:** raw 64-byte signature, base64-encoded (standard or URL-safe, no padding required)

### Example (Node.js / tweetnacl)

```js
import nacl from 'tweetnacl';
import { Buffer } from 'node:buffer';

// secretKey is a 64-byte Solana keypair (first 32 = private, last 32 = public)
function signMessage(message, secretKey) {
  const msgBytes = Buffer.from(message, 'utf8');
  const sig = nacl.sign.detached(msgBytes, secretKey);
  return Buffer.from(sig).toString('base64');
}
```

### Example (Python / PyNaCl)

```python
from nacl.signing import SigningKey
import base64

def sign_message(message: str, secret_key_bytes: bytes) -> str:
    # secret_key_bytes: first 32 bytes of a 64-byte Solana keypair
    sk = SigningKey(secret_key_bytes[:32])
    signed = sk.sign(message.encode('utf-8'))
    return base64.b64encode(signed.signature).decode('ascii')
```

---

## Step 4 — Submit the Signed Challenge

All mutating endpoints share the same body envelope:

```json
{
  "message": "<original challenge message, exactly as returned>",
  "signature": "<base64 signature>",
  ...endpoint-specific fields...
}
```

### Register example

```
POST /v1/services/{wallet}/register
Content-Type: application/json

{
  "message": "x402 subscription auth v1\n\naction: register\n...\nhmac_sha256_hex: ...\n",
  "signature": "base64...",
  "service_id": "api.myapp.com",
  "service_url": "https://api.myapp.com",
  "resources_allowlist": ["/api/v1/data"]
}
```

> The server re-derives the HMAC and checks that `service_id`, `service_url`,
> and `resources_allowlist` in the POST body match what was bound in the
> challenge message. Submitting different values than what you signed is rejected.

---

## Nonce / Replay Protection

- Each nonce is single-use. After a successful action, the nonce is consumed.
- Nonces expire after **10 minutes**.
- A wallet may have at most **10 outstanding (unconsumed) nonces** at a time.
  Requesting more returns a `400 Bad Request`.

---

## HMAC Construction (for client-side verification)

The HMAC is computed server-side and embedded in the challenge. Clients do not
need to verify it — the server checks it on submission. For implementers who
want to verify the challenge is authentic before signing:

```
preimage = join all message lines up to (not including) the blank line
           before "hmac_sha256_hex:", separated by "\n"
           + a trailing "\n"

hmac = HMAC-SHA256(key=SUBSCRIPTION_AUTH_HMAC_SECRET, data=preimage)
       expressed as lowercase hex
```

---

## Error Codes

| HTTP | `error` field         | Meaning                                                      |
|------|-----------------------|--------------------------------------------------------------|
| 400  | `BAD_REQUEST`         | Missing / malformed params                                   |
| 401  | `UNAUTHORIZED`        | Invalid signature, HMAC, nonce expired, or action mismatch  |
| 403  | `FORBIDDEN`           | Operation not allowed (e.g. not the service merchant)        |
| 404  | `NOT_FOUND`           | Resource not found                                           |
| 409  | `CONFLICT`            | Already exists (idempotent re-register)                      |
| 503  | `SERVICE_UNAVAILABLE` | Database not configured / unavailable                        |
| 500  | `INTERNAL_ERROR`      | Server error                                                 |

---

## Rate Limits

| Limit                        | Value         |
|------------------------------|---------------|
| Max outstanding nonces/wallet | 10            |
| Nonce TTL                    | 10 minutes    |
| Revocation feed window       | 7 days (configurable via `SUBSCRIPTION_AUTH_REVOCATION_WINDOW_DAYS`) |
| Subscriptions page size      | 50 per request (use `next_cursor` to paginate)                        |
