# Gating Your API with PR402 Subscriptions — For Dummies

**Simple is Best, yet Elegant.**

Got an existing web API? Want to charge users hourly/daily/monthly access? This guide shows you exactly how — **no blockchain knowledge needed.**

---

## What You'll Build

**Before:**
```
Your API → Anyone can call for free
```

**After:**
```
Your API → Only paid subscribers can call
           ↓
        Pay once/hour (or /day /month)
           ↓
        Get JWT token
           ↓
        Call API with token until it expires
```

**User experience:** Pay **once** for the time window, use your API unlimited during that period. No per-request payments.

---

## Two Ways: Quick Start vs. Production

### Path 1: Quick Start (5 minutes)
**Best for:** Testing, single service, getting started fast  
**How:** Your server signs JWT tokens locally  
**Setup:** Add 3 environment variables, done

### Path 2: Production (15 minutes)
**Best for:** Multiple services, central management, professional setup  
**How:** Miraland-labs' centralized JWT signing service (like Auth0)  
**Setup:** Register your service once, configure seller SDK

**This guide covers both.** Start with Path 1 today, switch to Path 2 when ready.

---

## Prerequisites

What you need:
- [ ] An existing web API (any language: Node.js, Python, Go, Rust, etc.)
- [ ] A Solana wallet with some SOL (devnet is free)
- [ ] 10 minutes

What you DON'T need:
- ❌ Deep blockchain knowledge
- ❌ Smart contract experience  
- ❌ Cryptocurrency exchange account (use devnet for testing)

---

## Path 1: Quick Start (Local JWT)

### Step 1: Install the seller SDK

**Node.js/TypeScript:**
```bash
npm install @pr402/subscription-seller
```

**Other languages:** HTTP API (examples below)

### Step 2: Set environment variables

```bash
# Generate JWT secret (any 32+ character string)
JWT_SECRET="your-secret-key-min-32-chars-long-here"

# Your Solana wallet address (where you receive payments)
MERCHANT_WALLET="YourWalletAddressHere..."

# PR402 facilitator (devnet for testing)
FACILITATOR_BASE_URL="https://preview.ipay.sh"

# Pricing (example: 0.1 USDC per hour)
X402_PAY_TO="EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"  # USDC mint
X402_AMOUNT_HOURLY="100000"  # 0.1 USDC (6 decimals)
```

### Step 3: Add subscription endpoint

This is where users **pay** for access:

**Node.js/Express:**
```javascript
const { createSubscriptionHandler } = require('@pr402/subscription-seller');

// Configure once at startup
const subscriptionHandler = createSubscriptionHandler({
  jwtSecret: process.env.JWT_SECRET,
  merchantWallet: process.env.MERCHANT_WALLET,
  facilitatorBaseUrl: process.env.FACILITATOR_BASE_URL,
  tiers: {
    hourly: {
      mint: process.env.X402_PAY_TO,
      amount: process.env.X402_AMOUNT_HOURLY,
      duration: 3600  // 1 hour in seconds
    }
  }
});

// Subscribe endpoint - users POST here to pay
app.post('/api/v1/subscribe', subscriptionHandler);
```

**Other languages (HTTP):**
```
POST /api/v1/subscribe
Content-Type: application/json

{
  "tier": "hourly",
  "payer": "BuyerWalletAddress...",
  "paymentPayload": { /* pr402 payment proof */ }
}

→ Verify payment with facilitator
→ Sign JWT token
→ Return: { "token": "eyJ...", "expiresAt": "2026-07-02T15:30:00Z" }
```

### Step 4: Protect your existing API routes

Add JWT verification to your API endpoints:

**Node.js/Express:**
```javascript
const jwt = require('jsonwebtoken');

// Middleware to protect routes
function requireSubscription(req, res, next) {
  const token = req.headers.authorization?.replace('Bearer ', '');
  
  if (!token) {
    return res.status(401).json({ error: 'No token provided' });
  }
  
  try {
    const decoded = jwt.verify(token, process.env.JWT_SECRET);
    
    // Check if expired
    if (Date.now() / 1000 >= decoded.exp) {
      return res.status(401).json({ error: 'Token expired, please resubscribe' });
    }
    
    req.user = decoded;  // Add subscriber info to request
    next();
  } catch (err) {
    return res.status(401).json({ error: 'Invalid token' });
  }
}

// Apply to your API routes
app.get('/api/v1/data', requireSubscription, (req, res) => {
  // Your existing API logic here
  res.json({ data: 'secret info', subscriber: req.user.payer });
});
```

**Python/Flask:**
```python
import jwt
from functools import wraps

def require_subscription(f):
    @wraps(f)
    def decorated(*args, **kwargs):
        token = request.headers.get('Authorization', '').replace('Bearer ', '')
        
        if not token:
            return jsonify({'error': 'No token provided'}), 401
        
        try:
            decoded = jwt.decode(token, os.getenv('JWT_SECRET'), algorithms=['HS256'])
            
            if time.time() >= decoded['exp']:
                return jsonify({'error': 'Token expired, please resubscribe'}), 401
            
            request.user = decoded
            return f(*args, **kwargs)
        except:
            return jsonify({'error': 'Invalid token'}), 401
    
    return decorated

@app.route('/api/v1/data')
@require_subscription
def get_data():
    return jsonify({'data': 'secret info', 'subscriber': request.user['payer']})
```

### Step 5: Test it!

**Terminal 1 - Start your server:**
```bash
npm start  # or python app.py
```

**Terminal 2 - Test subscription:**
```bash
# Install test client (or use your own)
npx @pr402/subscription-client subscribe \
  --api-url http://localhost:3000 \
  --tier hourly \
  --keypair /path/to/buyer-wallet.json

# Call your protected API
curl -H "Authorization: Bearer eyJ..." \
  http://localhost:3000/api/v1/data
```

**Done!** You now have a subscription-gated API. 🎉

---

## Path 2: Production Setup (Hosted Auth)

Why upgrade:
- ✅ **Professional:** RS256 tokens (like Auth0, not shared secrets)
- ✅ **Scalable:** One auth service for multiple APIs
- ✅ **Revocable:** Cancel subscriptions instantly
- ✅ **JWKS:** Standard token verification

**The mistake in the previous version**: Path 2 does NOT mean you deploy your own auth service. It means you use **miraland-labs' centralized JWT signing service** at https://auth.ipay.sh

### Step 1: Register your API service

**One-time setup** per API you want to protect:

```bash
# Clone the repo just to get the scripts
git clone https://github.com/miralandlabs/subscription-auth
cd subscription-auth/scripts
npm install

# Register with miraland-labs' centralized service
node register-service.mjs \
  --keypair /path/to/your-seller-wallet.json \
  --base-url https://auth.ipay.sh \
  --service-id api.myproduct.com \
  --service-url https://api.myproduct.com \
  --resources '["*"]'
```

**What this does:**
- Registers `api.myproduct.com` as your service ID with our central service
- Links it to your wallet (only you can issue tokens for it)
- Sets allowed resources (routes the token can access)

**No deployment needed!** We operate the auth service for you.

### Step 2: Configure your API server

Update environment variables:

```bash
# Switch to Tier B
SUBSCRIPTION_MODE=tier-b

# Point to miraland-labs' centralized auth service
SUBSCRIPTION_AUTH_BASE_URL=https://auth.ipay.sh
SUBSCRIPTION_AUTH_ISS=https://auth.ipay.sh
SUBSCRIPTION_AUTH_SERVICE_ID=api.myproduct.com

# Your wallet secret key (base58) - to sign challenges
SUBSCRIPTION_AUTH_MERCHANT_SECRET_KEY=your-base58-secret-key

# Revocation check interval
REVOCATION_POLL_INTERVAL_SEC=60

# Pricing (same as Path 1)
FACILITATOR_BASE_URL=https://preview.ipay.sh
X402_PAY_TO=EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v
X402_AMOUNT_HOURLY=100000
```

### Step 3: Update subscription endpoint

**Node.js with SDK:**
```javascript
const { 
  issueTokenViaAuthService,
  verifyTokenWithJwks 
} = require('@pr402/subscription-seller');

app.post('/api/v1/subscribe', async (req, res) => {
  const { tier, payer, paymentPayload } = req.body;
  
  // 1. Verify payment with facilitator (same as Path 1)
  const paymentVerified = await verifyPayment(paymentPayload);
  if (!paymentVerified) {
    return res.status(402).json({ error: 'Payment failed' });
  }
  
  // 2. Request RS256 token from miraland-labs' central auth service
  const result = await issueTokenViaAuthService({
    baseUrl: 'https://auth.ipay.sh',  // miraland-labs' service
    merchantWallet: process.env.MERCHANT_WALLET,
    serviceId: process.env.SUBSCRIPTION_AUTH_SERVICE_ID,
    payer,
    tier,
    resources: ['*'],
    signMessage: async (msg) => {
      // Sign challenge with your wallet to prove ownership
      return signMessageWithWallet(msg, process.env.SUBSCRIPTION_AUTH_MERCHANT_SECRET_KEY);
    }
  });
  
  // 3. Return token to buyer
  res.json({
    token: result.token,
    jti: result.jti,
    expiresAt: new Date(Date.now() + 3600000).toISOString(),
    persistenceHint: 'save-until-expire'
  });
});
```

### Step 4: Update JWT verification (JWKS)

**Node.js:**
```javascript
const jwksClient = require('jwks-rsa');
const jwt = require('jsonwebtoken');

// Create JWKS client to fetch public keys from miraland-labs' service
const client = jwksClient({
  jwksUri: 'https://auth.ipay.sh/.well-known/jwks.json',
  cache: true,
  cacheMaxAge: 600000  // 10 minutes
});

async function requireSubscription(req, res, next) {
  const token = req.headers.authorization?.replace('Bearer ', '');
  
  if (!token) {
    return res.status(401).json({ error: 'No token provided' });
  }
  
  try {
    // Decode header to get key ID
    const decoded = jwt.decode(token, { complete: true });
    const key = await client.getSigningKey(decoded.header.kid);
    
    // Verify RS256 signature against miraland-labs' public key
    const verified = jwt.verify(token, key.getPublicKey(), {
      issuer: 'https://auth.ipay.sh',
      algorithms: ['RS256']
    });
    
    // Check revocation (poll cache updates every 60s)
    const revoked = await revocationCache.isRevoked(verified.jti);
    if (revoked) {
      return res.status(401).json({ error: 'Token revoked' });
    }
    
    req.user = verified;
    next();
  } catch (err) {
    return res.status(401).json({ error: 'Invalid token' });
  }
}
```

**Done!** You now have production-grade subscription auth using miraland-labs' centralized service. 🎉

---

## Testing Your Setup

### Automated Test (Tier B)

```bash
cd scripts
node e2e-tier-b-auth.mjs \
  --keypair /path/to/seller-wallet.json
```

### Manual Test Flow

**1. Buyer pays for subscription:**
```bash
curl -X POST http://localhost:3000/api/v1/subscribe \
  -H "Content-Type: application/json" \
  -d '{
    "tier": "hourly",
    "payer": "BuyerWallet...",
    "paymentPayload": { /* pr402 payment */ }
  }'

# Returns:
{
  "token": "eyJhbGciOiJIUzI1NiIs...",
  "jti": "550e8400-e29b-41d4-a716-446655440000",
  "expiresAt": "2026-07-02T15:30:00Z",
  "tier": "hourly",
  "persistenceHint": "save-until-expire"
}
```

**2. Buyer uses token to call API:**
```bash
TOKEN="eyJhbGciOiJIUzI1NiIs..."

curl -H "Authorization: Bearer $TOKEN" \
  http://localhost:3000/api/v1/data

# Returns your protected data
```

**3. Buyer doesn't pay again until token expires**

---

## Common Patterns

### Multiple Tiers

Offer different pricing levels:

```javascript
const tiers = {
  hourly: {
    mint: USDC_MINT,
    amount: 100000,      // 0.1 USDC
    duration: 3600       // 1 hour
  },
  daily: {
    mint: USDC_MINT,
    amount: 2000000,     // 2 USDC
    duration: 86400      // 24 hours
  },
  monthly: {
    mint: USDC_MINT,
    amount: 50000000,    // 50 USDC
    duration: 2592000    // 30 days
  }
};
```

Users choose their tier:
```bash
POST /api/v1/subscribe
{ "tier": "daily", ... }
```

### Resource-Scoped Tokens

Limit which endpoints a token can access:

```javascript
// Issue token for specific resources
await issueTokenViaAuthService({
  // ...
  resources: ['/api/v1/data', '/api/v1/reports']
});

// Verify resource access in middleware
function requireSubscription(req, res, next) {
  // ... verify token ...
  
  const requestPath = req.path;
  const allowedResources = req.user.resources;
  
  // Check if wildcard or specific path allowed
  if (!allowedResources.includes('*') && 
      !allowedResources.includes(requestPath)) {
    return res.status(403).json({ error: 'Resource not allowed' });
  }
  
  next();
}
```

### Graceful Expiration

Help users know when to renew:

```javascript
app.get('/api/v1/data', requireSubscription, (req, res) => {
  const timeLeft = req.user.exp - Math.floor(Date.now() / 1000);
  
  res.json({
    data: 'your data here',
    subscription: {
      expiresIn: timeLeft,
      tier: req.user.tier,
      renewUrl: '/api/v1/subscribe'
    }
  });
});
```

### Early Revocation (Tier B only)

Cancel a subscription before it expires:

```bash
# Seller revokes token
node scripts/revoke-token.mjs \
  --keypair /path/to/seller-wallet.json \
  --jti 550e8400-e29b-41d4-a716-446655440000 \
  --service-id api.myproduct.com
```

Your API checks revocation automatically (60s poll).

---

## Troubleshooting

### "Payment verification failed"

**Cause:** PR402 facilitator couldn't verify the payment  
**Fix:** 
- Check `FACILITATOR_BASE_URL` is correct
- Verify payment includes all required fields
- Test with devnet USDC first

### "Token expired"

**Cause:** Buyer's token validity period ended  
**Fix:** Buyer needs to call `/api/v1/subscribe` again (pay for new period)

### "Invalid signature" (Tier B)

**Cause:** JWT signature doesn't match JWKS public key  
**Fix:**
- Verify `SUBSCRIPTION_AUTH_ISS` matches your deployment
- Check JWKS endpoint is accessible: `curl https://your-auth/.well-known/jwks.json`
- Ensure RSA key pair is correct

### "CORS error in browser"

**Cause:** Frontend calling API from different domain  
**Fix:** Add CORS headers to your API:
```javascript
app.use((req, res, next) => {
  res.header('Access-Control-Allow-Origin', '*');
  res.header('Access-Control-Allow-Headers', 'Authorization, Content-Type');
  next();
});
```

---

## Migration: Path 1 → Path 2

When you're ready to upgrade from Quick Start to Production:

**1. Register with miraland-labs' service** (Step 1 above)

**2. Update environment variables:**
```bash
# Change this:
SUBSCRIPTION_MODE=tier-a
JWT_SECRET=...

# To this:
SUBSCRIPTION_MODE=tier-b
SUBSCRIPTION_AUTH_BASE_URL=https://auth.ipay.sh
SUBSCRIPTION_AUTH_ISS=https://auth.ipay.sh
SUBSCRIPTION_AUTH_SERVICE_ID=api.myproduct.com
SUBSCRIPTION_AUTH_MERCHANT_SECRET_KEY=...
```

**3. Update code:** Replace local JWT signing with centralized auth service call (Step 3 above)

**4. Deploy:** No database migration needed, old tokens expire naturally

**Downtime:** Zero. Both modes use the same `/api/v1/subscribe` endpoint.

**Note:** You don't deploy anything yourself - you just register with and call miraland-labs' centralized service.

---

## Next Steps

✅ **You've gated your API!** Users now pay for time-based access.

**What's next:**

1. **Add analytics:** Track subscription revenue, popular tiers
2. **Add webhooks:** Notify your system when subscriptions happen
3. **Add trial periods:** Free hour before first payment
4. **Add team plans:** One payment, multiple users share token
5. **Add usage limits:** Rate limit per tier (hourly = 100 req/hour)

**Resources:**

- [Full API Reference](SUBSCRIPTION_AUTH_FOR_SELLERS.md)
- [Wire Protocol Details](PROTOCOL.md)
- [Example Code](https://github.com/miralandlabs/x402-subscription-starter)
- [Buyer SDK](https://github.com/miralandlabs/x402-subscription-client)

**Questions?** Check existing documentation or open an issue.

---

## Quick Reference Card

```
┌─────────────────────────────────────────────────────┐
│ SUBSCRIPTION FLOW                                   │
├─────────────────────────────────────────────────────┤
│ 1. Buyer: POST /api/v1/subscribe                   │
│    → Send: { tier, payer, paymentPayload }         │
│                                                     │
│ 2. Seller: Verify payment → Sign JWT               │
│    → Return: { token, expiresAt }                  │
│                                                     │
│ 3. Buyer: Save token, call APIs                    │
│    → Header: Authorization: Bearer <token>         │
│                                                     │
│ 4. Seller: Verify JWT on each API call             │
│    → Check: signature, expiration, revocation      │
│                                                     │
│ 5. When expired: Buyer repeats step 1              │
└─────────────────────────────────────────────────────┘

PAYMENT: Once per time window (hour/day/month)
API CALLS: Unlimited during window
TOKEN: Bearer JWT in Authorization header
SECURITY: RS256 or HS256, standard JWT verification
```

**Remember:** Simple is Best, yet Elegant. Start simple (Path 1), upgrade when ready (Path 2).
