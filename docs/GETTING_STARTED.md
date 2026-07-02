# 🚀 Gating an Existing Web2 API with pr402 Subscriptions

If you have an existing Web2 API (e.g., built with Express, FastAPI, Go, etc.) and want to add an hourly, daily, or monthly paywall, you have **two main paths** to choose from. 

Choose the one that fits your stack best:

---

## 💡 Pick Your Path

| Feature | Path 1: Local JWT (Tier A) | Path 2: Centralized Service (Tier B) |
|:---|:---|:---|
| **What it is** | You sign and verify subscription tokens locally on your own server. | You use a shared, hosted auth oracle (e.g. `https://preview.auth.ipay.sh` for testing) to sign RS256 tokens. |
| **Infrastructure** | 🟢 **None**. No database, no new servers. | 🟢 **None**. You call the public auth service API. |
| **Token Type** | HS256 (Symmetric, secret key). | RS256 (Asymmetric, public/private keys + JWKS). |
| **Best For** | Simple setup, fast integration into your existing server. | Advanced setups where third-party clients need to verify your tokens. |

> [!IMPORTANT]
> **Production vs. Development Base URLs (Path 2):**
> * Use **`https://preview.auth.ipay.sh`** exclusively for **integration testing and development**.
> * Once going live, you **must** update your base URLs to the production endpoint: **`https://auth.ipay.sh`**.

---

## 🔑 Prerequisite: Your Seller Wallet
For both paths, you need a Solana wallet keypair to act as your merchant identity.
* If you don't have one, generate it using the Solana CLI:
  ```bash
  solana-keygen new --outfile seller-keypair.json --no-bip39-passphrase
  ```

---

## 🧭 Path 1: Local JWT (Tier A) — Easiest & Fastest

You do not need to call any external auth services. You generate a random secret key and handle token signing and verification directly inside your existing Web2 code.

### 1. Configure your environment
Create a random 32-byte secret key and add it to your `.env` file along with your wallet public key:
```env
JWT_SECRET=your-random-32-byte-hex-secret-key-here
MERCHANT_WALLET=your-solana-wallet-public-key
```

### 2. Implementation (Node.js/Express Example)
Install `jsonwebtoken` (or similar for your language):
```bash
npm install jsonwebtoken dotenv
```

Drop this into your existing API codebase:

```javascript
import dotenv from 'dotenv';
import jwt from 'jsonwebtoken';
dotenv.config();

const JWT_SECRET = process.env.JWT_SECRET;

// 1. CALL THIS AFTER PAYMENT SETTLEMENT
// Call this when the buyer completes the pr402 payment flow.
export function issueSubscriptionToken(buyerWallet, tier = 'monthly') {
  const durations = {
    hourly: 60 * 60,
    daily: 24 * 60 * 60,
    monthly: 30 * 24 * 60 * 60,
  };

  const seconds = durations[tier] || durations.monthly;

  return jwt.sign(
    { 
      payer: buyerWallet,
      tier: tier 
    }, 
    JWT_SECRET, 
    { expiresIn: seconds }
  );
}

// 2. EXPRESS MIDDLEWARE TO SECURE YOUR ROUTE
export function requireSubscription(req, res, next) {
  const authHeader = req.headers['authorization'];
  if (!authHeader?.startsWith('Bearer ')) {
    return res.status(401).json({ error: 'MISSING_TOKEN', message: 'Bearer token required' });
  }

  const token = authHeader.slice(7);

  try {
    const decoded = jwt.verify(token, JWT_SECRET);
    req.subscription = decoded; // Attach subscription info to the request
    next();
  } catch (err) {
    return res.status(401).json({ error: 'TOKEN_EXPIRED_OR_INVALID', message: err.message });
  }
}
```

---

## 🌐 Path 2: Centralized Service (Tier B) — Shared Hosted Instance

You use the shared public deployment of the `subscription-auth` service.
* **Dev/Testing**: Use `https://preview.auth.ipay.sh` (devnet).
* **Production**: Use `https://auth.ipay.sh` (mainnet).

The service signs tokens using a private RSA key, and your server verifies them using the public JWKS endpoint.

### 1. Register your service once
You need to let the centralized service know your `service_id` and wallet. Since the helper registration scripts are in this repository, clone the repo locally to run the one-time registration:

```bash
# Clone the repository locally
git clone https://github.com/miraland-labs/x402.git
cd x402/subscription-auth/scripts
npm install

# Register your product (Replace URL with https://auth.ipay.sh for production)
node register-service.mjs \
  --keypair /path/to/your/seller-keypair.json \
  --base-url https://preview.auth.ipay.sh \
  --service-id api.myproduct.com \
  --service-url https://api.myproduct.com \
  --allowlist "/api/v1/data"
```
*(Replace `api.myproduct.com` with your API domain name, or use `YourWalletAddress:appName` if you don't have a domain).*

### 2. Configure your Web2 API environment
Add these values to your existing server's `.env` file:
```env
# Switch these URLs to https://auth.ipay.sh when going production!
SUBSCRIPTION_AUTH_BASE_URL=https://preview.auth.ipay.sh
SUBSCRIPTION_AUTH_ISS=https://preview.auth.ipay.sh
SUBSCRIPTION_AUTH_SERVICE_ID=api.myproduct.com
MERCHANT_WALLET=your-registered-wallet-public-key
MERCHANT_KEYPAIR_PATH=/path/to/your/seller-keypair.json
```

### 3. Implementation (Node.js/Express Example)
Install dependencies in your API project:
```bash
npm install @pr402/subscription-seller jose tweetnacl dotenv
```

Drop the module from our examples directory ([**`examples/minimal-seller/subscription.js`**](../examples/minimal-seller/subscription.js)) directly into your codebase. It contains:
- `issueToken({ payer, tier })`: Contacts the centralized auth service to fetch a signed token.
- `requireSubscription`: Middleware that validates the incoming token against the public JWKS endpoint (and polls for revocations automatically every 60 seconds).
- `revokeToken(jti)`: Instantly blocks a token.

Secure your endpoint:
```javascript
import { requireSubscription } from './subscription.js';

app.get('/api/v1/data', requireSubscription, (req, res) => {
  res.json({
    message: "Here is your paid data!",
    buyerWallet: req.subscription.payer
  });
});
```

---

## 🧪 Testing your Integration
Once either path is implemented, you can verify your Express route is locked by testing it using curl:

```bash
# 1. Accessing without a token should fail (401)
curl http://localhost:3000/api/v1/data
# Expected: {"error":"MISSING_TOKEN", ...}

# 2. Accessing with a valid token should pass (200)
curl -H "Authorization: Bearer <your_jwt_here>" http://localhost:3000/api/v1/data
```
