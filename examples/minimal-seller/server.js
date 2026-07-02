import express from 'express';
import { issueToken, requireSubscription, revokeToken } from './subscription.js';

const app = express();
app.use(express.json());

// PORT defaults to 3000
const PORT = process.env.PORT || 3000;

// In-memory "database" to store issued JTIs so you can play with revocation
const activeTokens = [];

// 1. Home page / instructions
app.get('/', (req, res) => {
  res.send(`
    <html>
      <body style="font-family: sans-serif; max-width: 600px; margin: 40px auto; line-height: 1.6;">
        <h2>Minimal Subscription Seller Demo</h2>
        <p>This Express server demonstrates how to issue, verify, and revoke tokens via <code>subscription-auth</code>.</p>
        
        <h3>Step 1: Get a Token</h3>
        <p>Send a POST request to simulated subscribe page:</p>
        <pre style="background: #f4f4f4; padding: 10px;">
POST /pay-and-subscribe
Content-Type: application/json

{
  "payer": "buyA5hR1Z9KtHQRBTmLkjsFfjAabDwdZtrRC6edqxAJ"
}</pre>

        <h3>Step 2: Use the Token</h3>
        <p>Access the protected data endpoint with the returned token:</p>
        <pre style="background: #f4f4f4; padding: 10px;">
GET /api/data
Authorization: Bearer &lt;paste_token_here&gt;</pre>

        <h3>Step 3: Revoke the Token</h3>
        <p>Revoke the token to block access:</p>
        <pre style="background: #f4f4f4; padding: 10px;">
POST /revoke
Content-Type: application/json

{
  "jti": "&lt;paste_jti_here&gt;"
}</pre>
      </body>
    </html>
  `);
});

// 2. Simulated payment/subscribe endpoint
app.post('/pay-and-subscribe', async (req, res) => {
  const { payer } = req.body;
  if (!payer) {
    return res.status(400).json({ error: 'MISSING_PAYER', message: 'Specify buyer wallet public key as "payer"' });
  }

  try {
    console.log(`[server] Received subscription request from payer: ${payer}`);
    
    // In a real app, you would check the Solana/Miraland transaction here.
    // Once payment is settled, we issue the token:
    const tokenInfo = await issueToken({
      payer,
      tier: 'monthly',
      resources: ['/api/v1/data'] // must match a subset of service allowlist
    });

    activeTokens.push({ jti: tokenInfo.jti, payer });

    res.json({
      success: true,
      message: 'Subscription successful!',
      token: tokenInfo.token,
      jti: tokenInfo.jti,
      expiresAt: tokenInfo.expiresAt,
      activeTokensList: activeTokens
    });
  } catch (err) {
    console.error('[server] Error issuing token:', err);
    res.status(500).json({ error: 'TOKEN_ISSUANCE_FAILED', message: err.message });
  }
});

// 3. Protected endpoint (requires subscription)
app.get('/api/data', requireSubscription, (req, res) => {
  // If requireSubscription middleware succeeds, req.subscription is populated
  res.json({
    success: true,
    data: '🔓 Secret gold-member premium content accessed successfully!',
    subscriptionInfo: req.subscription
  });
});

// 4. Revocation endpoint
app.post('/revoke', async (req, res) => {
  const { jti } = req.body;
  if (!jti) {
    return res.status(400).json({ error: 'MISSING_JTI', message: 'jti is required to revoke' });
  }

  try {
    console.log(`[server] Revoking token jti: ${jti}`);
    await revokeToken(jti);
    
    // Remove from our in-memory list
    const index = activeTokens.findIndex(t => t.jti === jti);
    if (index !== -1) activeTokens.splice(index, 1);

    res.json({
      success: true,
      message: `Token ${jti} revoked. Access will be blocked in ~60 seconds (once the revocation poll runs).`,
      activeTokensList: activeTokens
    });
  } catch (err) {
    console.error('[server] Error revoking token:', err);
    res.status(500).json({ error: 'REVOCATION_FAILED', message: err.message });
  }
});

app.listen(PORT, () => {
  console.log(`\n🚀 Minimal seller server running at http://localhost:${PORT}`);
  console.log(`Make sure your subscription-auth service is running and accessible!\n`);
});
