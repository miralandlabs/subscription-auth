/**
 * subscription.js — drop-in module for integrating subscription-auth into any Node.js server.
 *
 * Three things this file gives you:
 *   1. issueToken(payer, tier, resources) — call after payment to create a JWT
 *   2. requireSubscription               — Express middleware that verifies JWTs
 *   3. revokeToken(jti)                  — call to cut off access early
 *
 * Revocation polling runs automatically in the background every 60 seconds.
 * Copy this file into your project and set the env vars in .env.
 */

import 'dotenv/config';
import { readFileSync } from 'node:fs';
import { createRemoteJWKSet, jwtVerify } from 'jose';
import nacl from 'tweetnacl';
import {
  issueTokenViaAuthService,
  revokeToken as sdkRevokeToken,
  createRevocationPollCache,
} from '@pr402/subscription-seller';

// ─── Configuration — loaded from environment ─────────────────────────────────

const BASE_URL   = process.env.SUBSCRIPTION_AUTH_BASE_URL;
const ISS        = process.env.SUBSCRIPTION_AUTH_ISS;
const SERVICE_ID = process.env.SUBSCRIPTION_AUTH_SERVICE_ID;
const WALLET     = process.env.MERCHANT_WALLET;
const KEYPAIR    = process.env.MERCHANT_KEYPAIR_PATH;

for (const [k, v] of Object.entries({ BASE_URL, ISS, SERVICE_ID, WALLET, KEYPAIR })) {
  if (!v) throw new Error(`Missing env var: ${k} — check your .env file`);
}

// ─── Load signing key ─────────────────────────────────────────────────────────

// The keypair is a 64-byte array: first 32 = private, last 32 = public.
// This is the standard Solana keypair format.
let _secretKey;
try {
  _secretKey = Uint8Array.from(JSON.parse(readFileSync(KEYPAIR, 'utf8')));
} catch (err) {
  throw new Error(`Cannot read keypair file at "${KEYPAIR}": ${err.message}`);
}

/**
 * Signs a challenge message with your Ed25519 seller wallet.
 * The subscription-auth server verifies this signature to confirm you own the wallet.
 */
async function signMessage(message) {
  const sig = nacl.sign.detached(Buffer.from(message, 'utf8'), _secretKey);
  return Buffer.from(sig).toString('base64');
}

// ─── JWKS — used to verify buyer tokens on every request ─────────────────────

// Fetched once and cached. Keys are rotated automatically when the kid changes.
const JWKS = createRemoteJWKSet(new URL(`${BASE_URL}/.well-known/jwks.json`));

// ─── Revocation cache — background poll every 60s ────────────────────────────

const _revocationCache = createRevocationPollCache({
  baseUrl:     BASE_URL,
  serviceId:   SERVICE_ID,
  intervalSec: 60,
});

// Start polling immediately. Fail-open: if the poll fails, old cache is kept.
_revocationCache.start();
console.log(`[subscription] Revocation polling started (every 60s) for ${SERVICE_ID}`);

// ─── Public API ───────────────────────────────────────────────────────────────

/**
 * Issue a JWT for a buyer who just paid.
 *
 * @param {object} opts
 * @param {string} opts.payer     - Buyer's Solana wallet public key
 * @param {string} opts.tier      - 'hourly' | 'daily' | 'weekly' | 'monthly' | 'yearly'
 * @param {string[]} opts.resources - URL paths to grant access to. Must be a subset
 *                                    of the allowlist you set when registering.
 *                                    e.g. ['/api/v1/data', '/api/v1/feed']
 * @returns {{ token: string, jti: string, expiresAt: string }}
 */
export async function issueToken({ payer, tier = 'monthly', resources = ['/api/v1/data'] }) {
  if (!payer) throw new Error('issueToken: payer is required');

  const result = await issueTokenViaAuthService({
    baseUrl:        BASE_URL,
    merchantWallet: WALLET,
    serviceId:      SERVICE_ID,
    payer,
    tier,
    resources,
    signMessage,
  });

  console.log(`[subscription] Issued token jti=${result.jti} payer=${payer} tier=${tier} exp=${result.expiresAt}`);
  return result; // { token, jti, tier, tierLabel, expiresAt, service_id, resources }
}

/**
 * Express middleware — protect a route with subscription auth.
 *
 * Usage:
 *   app.get('/api/data', requireSubscription, (req, res) => {
 *     // req.subscription.payer  = buyer wallet
 *     // req.subscription.tier   = 'monthly'
 *     // req.subscription.jti    = token id (for revocation)
 *     res.json({ data: 'secret stuff' });
 *   });
 */
export async function requireSubscription(req, res, next) {
  const authHeader = req.headers['authorization'];

  if (!authHeader?.startsWith('Bearer ')) {
    return res.status(401).json({
      error: 'MISSING_TOKEN',
      message: 'Include your subscription token as: Authorization: Bearer <token>',
    });
  }

  const token = authHeader.slice(7);

  try {
    // 1. Verify signature, expiry, issuer
    const { payload } = await jwtVerify(token, JWKS, {
      issuer:     ISS,
      algorithms: ['RS256'],
    });

    // 2. Check revocation
    const isRevoked = await _revocationCache.isRevoked(payload.jti);
    if (isRevoked) {
      return res.status(401).json({
        error: 'TOKEN_REVOKED',
        message: 'This token has been revoked. Please re-subscribe.',
      });
    }

    // 3. Attach subscription info to the request for downstream handlers
    req.subscription = {
      jti:       payload.jti,
      payer:     payload.payer,
      tier:      payload.tier,
      serviceId: payload.sub,
      expiresAt: new Date(payload.exp * 1000).toISOString(),
    };

    next();
  } catch (err) {
    // Common errors: token expired, bad signature, wrong issuer
    const isExpired = err.code === 'ERR_JWT_EXPIRED';
    return res.status(401).json({
      error: isExpired ? 'TOKEN_EXPIRED' : 'INVALID_TOKEN',
      message: isExpired
        ? 'Subscription expired. Please re-subscribe.'
        : `Token verification failed: ${err.message}`,
    });
  }
}

/**
 * Revoke a specific token (e.g. when buyer requests a refund or cancels).
 * Takes effect within the next revocation poll interval (~60s).
 *
 * @param {string} jti - The token ID returned by issueToken
 */
export async function revokeToken(jti) {
  await sdkRevokeToken({
    baseUrl:        BASE_URL,
    merchantWallet: WALLET,
    serviceId:      SERVICE_ID,
    jti,
    signMessage,
  });
  console.log(`[subscription] Revoked token jti=${jti}`);
}
