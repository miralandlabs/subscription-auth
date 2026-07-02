# 🚀 subscription-auth — Easy Step-by-Step Guide for New Sellers

If you're a new seller and cryptographically signing JWTs sounds confusing, don't worry. This guide is built for you. 

---

## 💡 Do I really need to deploy this service? (Pick your path)

Before you touch any code, choose the path that fits your existing setup:

| Path | Difficulty | Extra Infrastructure | What you need to do |
|:---|:---|:---|:---|
| **1. Tier A (Local JWT)** | ⭐ *Easiest* | **None** (No Postgres, No Vercel) | Sign and verify tokens directly inside your existing Web2 server using a local secret key. |
| **2. Shared Tier B (Hosted)** | ⭐⭐ *Easy* | **None** (No Postgres, No Vercel) | Register your service on a shared public auth service deployment (e.g., `https://preview.auth.ipay.sh`) and call their API to issue tokens. |
| **3. Private Tier B (Self-Hosted)** | ⭐⭐⭐ *Advanced* | **Postgres DB + Vercel Account** | Deploy this repository to your own Vercel account and connect it to your own Postgres database. |

*If you are an existing Web2 developer who just wants to add a gate quickly, choose **Path 1** or **Path 2**. You do not need to host this repository or set up a Postgres database!*

---

## 🛠️ Prerequisites (What you need)

### For Path 1 & 2 (Easiest / No Server Deployment):
1. **Node.js (v18+)** installed on your computer.
2. A **Solana Keypair File** (a `.json` file containing a 64-byte array representing your seller wallet).
   * Create one quickly if you don't have it:
     ```bash
     solana-keygen new --outfile seller-keypair.json --no-bip39-passphrase
     ```

### For Path 3 (Self-Hosting your own Auth Server):
1. All items above, plus:
2. A **Postgres Database** (e.g. free tier from [Neon.tech](https://neon.tech) or [Supabase](https://supabase.com)).
3. A **Vercel account** (free tier from [vercel.com](https://vercel.com)).


---

## 🏁 Step-by-Step Setup

Decide whether you want to use a **Shared Hosted Instance** or **Self-Host** your own auth service.

---

### 🌐 Option A: Use a Shared/Hosted Instance (Fastest, no DB/Vercel needed)
If you do not want to set up Vercel or Postgres, you can use the public testing deployment: `https://preview.auth.ipay.sh` (or any other facilitator-provided deployment).
1. Skip directly to **Step 4** (Register your Service).
2. When running scripts or starting your Express server, use `https://preview.auth.ipay.sh` as your `SUBSCRIPTION_AUTH_BASE_URL`.

---

### 🖥️ Option B: Self-Host your own Auth Server (Requires DB + Vercel)
Follow these three steps only if you chose **Path 3** (Self-Hosting your own private oracle):

#### Step 1: Run the Setup Wizard
Open your terminal, go to the project directory, and run the guided setup script:

```bash
cd subscription-auth
bash scripts/setup.sh --iss https://YOUR-VERCEL-DEPLOYMENT.vercel.app
```
*(Replace `https://YOUR-VERCEL-DEPLOYMENT.vercel.app` with the URL you expect Vercel to give you. If you don't know it yet, you can use `https://my-auth-service.vercel.app` and update it later).*

**What this does**:
* Generates a secure RSA private key (`subscription-auth-private.pem`) in your root directory.
* Generates a random HMAC secret.
* Prints a block of environment variables. **Keep this open in your terminal!**

#### Step 2: Initialize your Postgres Database
1. Go to your Neon/Supabase dashboard and copy your **connection string** (it looks like `postgresql://user:pass@host/db?sslmode=require`).
2. Run this command to create the required tables:
   ```bash
   psql "YOUR_POSTGRES_CONNECTION_STRING" -f migrations/init.sql
   ```
*(If you don't have `psql` installed, you can open your SQL editor in Neon/Supabase, copy the contents of `migrations/init.sql`, and run it as a query).*

#### Step 3: Deploy to Vercel
Deploy the `subscription-auth` service so it's live on the internet:

1. Install the Vercel CLI (if you haven't already):
   ```bash
   npm install -g vercel
   ```
2. Run the deployment:
   ```bash
   vercel
   ```
3. When prompted, link it to your Vercel account.
4. **Important**: Add the environment variables that the setup script printed in **Step 1** to your Vercel dashboard (**Settings -> Environment Variables**):
   * `DATABASE_URL` (your Postgres connection string)
   * `SUBSCRIPTION_AUTH_HMAC_SECRET`
   * `SUBSCRIPTION_AUTH_RSA_PRIVATE_KEY_PEM`
   * `SUBSCRIPTION_AUTH_KEY_ID`
   * `SUBSCRIPTION_AUTH_ISS`
5. Deploy to production:
   ```bash
   vercel --prod
   ```

Copy your live deployment URL (e.g., `https://subscription-auth-xyz.vercel.app`).


---

### Step 4: Register your Service (One-Time)
Now register your service on your new deployment. This tells the auth service: *"My server at `api.myproduct.com`, owned by my wallet, is allowed to request tokens."*

Run the registration script:

```bash
# 1. Install script dependencies
cd scripts && npm install && cd ..

# 2. Run the registration
# (If using Option A, replace base-url with https://preview.auth.ipay.sh)
node scripts/register-service.mjs \
  --keypair /path/to/your/seller-keypair.json \
  --base-url https://YOUR-VERCEL-DEPLOYMENT.vercel.app \
  --service-id api.myproduct.com \
  --service-url https://api.myproduct.com \
  --allowlist "/api/v1/data,/api/v1/feed"
```

> 💡 **What is a `service-id`?** It's a namespace for your product. To prevent people from stealing names, it *must* have a dot or colon in it (like a domain name or `walletAddress:appName`). Flat names like `mycoolapi` will be rejected.
>
> 💡 **What is an `allowlist`?** A comma-separated list of API endpoints users can access with this token. If you aren't sure, use `"*"` (allows everything).

---

### Step 5: Test the Integration with our Minimal Seller Server
We've built a mock Express server in `examples/minimal-seller/` so you can see how everything hooks together.

1. Go to the example folder:
   ```bash
   cd examples/minimal-seller
   ```
2. Install dependencies:
   ```bash
   npm install
   ```
3. Copy `.env.example` to `.env`:
   ```bash
   cp .env.example .env
   ```
4. Open the `.env` file and fill in your details:
   * `SUBSCRIPTION_AUTH_BASE_URL` (your Vercel deployment URL)
   * `SUBSCRIPTION_AUTH_ISS` (same as above)
   * `SUBSCRIPTION_AUTH_SERVICE_ID` (`api.myproduct.com` or whatever you registered)
   * `MERCHANT_KEYPAIR_PATH` (absolute path to your Solana `seller-keypair.json`)
   * `MERCHANT_WALLET` (your public wallet key)
5. Start the mock server:
   ```bash
   npm run dev
   ```

Open your browser to `http://localhost:3000`. You can follow the interactive buttons on screen to:
1. **Simulate a Payment**: Clicking this calls the auth service to issue a token.
2. **Access Protected Data**: Try accessing the data route with and without the token to see Express middleware verification in action.
3. **Revoke a Token**: Revoke the token to witness the backend pull feed blocking access.

---

## 📝 Integration Cheatsheet (For your production app)

When you write your actual application, copy the pattern in `examples/minimal-seller/subscription.js`:

### 1. Issuing a token when a buyer pays
After you verify that a user has paid, call the auth service to issue a token:

```js
import { issueToken } from './subscription.js';

app.post('/api/subscribe', async (req, res) => {
  // 1. Process payment (e.g. via Miraland/Solana)
  // ...
  
  // 2. Issue the JWT
  const tokenInfo = await issueToken({
    payer: req.body.payerWallet, 
    tier: 'monthly',
    resources: ['/api/v1/data'] // Must match a subset of your allowlist
  });

  // 3. Return token to the client
  res.json({ token: tokenInfo.token });
});
```

### 2. Protecting your API routes
Secure any data route using the verification middleware:

```js
import { requireSubscription } from './subscription.js';

app.get('/api/v1/data', requireSubscription, (req, res) => {
  // Access is only granted if a valid, non-revoked token is in the Authorization header
  res.json({
    message: "Here is your paid data!",
    buyerWallet: req.subscription.payer
  });
});
```

---

## ❓ FAQ & Troubleshooting

#### "Error: RSA encoding key"
Your `SUBSCRIPTION_AUTH_RSA_PRIVATE_KEY_PEM` env var in Vercel is formatted incorrectly. The newlines must be converted to `\n` so it fits on a single line. Re-run `bash scripts/setup.sh` to get the pre-formatted copy-paste line.

#### "Conflict: service_id already registered"
This is totally fine. It means you (or someone else) already registered that `service_id`. You don't need to register it again. Move directly to starting your server.

#### "too many pending challenges"
To prevent database spamming, a wallet can only have 10 active login challenges outstanding at once. Wait 10 minutes for them to expire, then try again.

#### "Token validation failed: JWT Expired"
The buyer's token has expired. Prompt them to make a payment and fetch a new token via your `/pay-and-subscribe` endpoint.
