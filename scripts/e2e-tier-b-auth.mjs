#!/usr/bin/env node
/**
 * Auth-only Tier B E2E against subscription-auth (no pr402 payment).
 * Uses @pr402/subscription-seller from npm in a temp install dir.
 *
 * Usage:
 *   node scripts/e2e-tier-b-auth.mjs \
 *     --keypair ../../demo-wallets/seller-keypair.json \
 *     [--payer buyA5hR1Z9KtHQRBTmLkjsFfjAabDwdZtrRC6edqxAJ] \
 *     [--revoke-only --jti <uuid>] \
 *     [--dry-run]
 *
 * Loads scripts/.env.local if present.
 */
import { readFileSync, existsSync, mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { tmpdir } from 'node:os';
import { execSync } from 'node:child_process';
import { createPrivateKey, sign as edSign } from 'node:crypto';

const __dirname = dirname(fileURLToPath(import.meta.url));

const BASE58_ALPHABET = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';

function base58Encode(bytes) {
  if (bytes.length === 0) return '';
  let zeros = 0;
  while (zeros < bytes.length && bytes[zeros] === 0) zeros += 1;
  const digits = [0];
  for (let i = zeros; i < bytes.length; i += 1) {
    let carry = bytes[i];
    for (let j = 0; j < digits.length; j += 1) {
      carry += digits[j] << 8;
      digits[j] = carry % 58;
      carry = (carry / 58) | 0;
    }
    while (carry > 0) {
      digits.push(carry % 58);
      carry = (carry / 58) | 0;
    }
  }
  let out = '';
  for (let i = 0; i < zeros; i += 1) out += '1';
  for (let i = digits.length - 1; i >= 0; i -= 1) out += BASE58_ALPHABET[digits[i]];
  return out;
}

function loadDotenvLocal() {
  const path = join(__dirname, '.env.local');
  if (!existsSync(path)) return;
  for (const line of readFileSync(path, 'utf8').split('\n')) {
    const t = line.trim();
    if (!t || t.startsWith('#')) continue;
    const i = t.indexOf('=');
    if (i === -1) continue;
    const k = t.slice(0, i).trim();
    let v = t.slice(i + 1).trim();
    if ((v.startsWith('"') && v.endsWith('"')) || (v.startsWith("'") && v.endsWith("'"))) {
      v = v.slice(1, -1);
    }
    if (process.env[k] === undefined) process.env[k] = v;
  }
}

function arg(name, fallback = undefined) {
  const i = process.argv.indexOf(`--${name}`);
  if (i === -1) return fallback;
  const v = process.argv[i + 1];
  return v && !v.startsWith('--') ? v : true;
}

loadDotenvLocal();

const keypairPath = arg('keypair', process.env.MERCHANT_KEYPAIR);
const baseUrl = (arg('base-url', process.env.SUBSCRIPTION_AUTH_BASE_URL) || 'https://preview.auth.ipay.sh').replace(/\/+$/, '');
const serviceId = arg('service-id', process.env.SUBSCRIPTION_AUTH_SERVICE_ID) || 'e2e.local.ipay.sh';
const expectedIss = (process.env.SUBSCRIPTION_AUTH_ISS || baseUrl).replace(/\/+$/, '');
const payer = arg('payer', process.env.TEST_PAYER) || 'buyA5hR1Z9KtHQRBTmLkjsFfjAabDwdZtrRC6edqxAJ';
const dryRun = arg('dry-run') === true;
const revokeOnly = arg('revoke-only') === true;
const jtiArg = arg('jti');

if (!keypairPath || keypairPath === true) {
  console.error('Required: --keypair <path>');
  process.exit(2);
}

function loadSigner(path) {
  const raw = JSON.parse(readFileSync(path, 'utf8'));
  const bytes = Uint8Array.from(raw);
  const seed = Buffer.from(bytes.slice(0, 32));
  const pkcs8 = Buffer.concat([Buffer.from('302e020100300506032b657004220420', 'hex'), seed]);
  return createPrivateKey({ key: pkcs8, format: 'der', type: 'pkcs8' });
}

function walletFromKeypair(path) {
  const raw = Uint8Array.from(JSON.parse(readFileSync(path, 'utf8')));
  return base58Encode(raw.slice(32, 64));
}

function signMessageB64(signer, message) {
  return edSign(null, Buffer.from(message, 'utf8'), signer).toString('base64');
}

async function fetchRevokeChallenge(wallet, jti) {
  const url = new URL(`/v1/services/${wallet}/challenge`, baseUrl);
  url.searchParams.set('action', 'revoke');
  url.searchParams.set('service_id', serviceId);
  url.searchParams.set('jti', jti);
  const res = await fetch(url.toString());
  const text = await res.text();
  if (!res.ok) throw new Error(`revoke challenge ${res.status}: ${text}`);
  return JSON.parse(text).message;
}

async function revokeToken(wallet, signer, jti) {
  const message = await fetchRevokeChallenge(wallet, jti);
  const signature = signMessageB64(signer, message);
  const res = await fetch(`${baseUrl}/v1/tokens/revoke`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ message, signature }),
  });
  const text = await res.text();
  if (!res.ok) throw new Error(`revoke ${res.status}: ${text}`);
  return JSON.parse(text);
}

async function introspect(token) {
  const res = await fetch(`${baseUrl}/v1/tokens/introspect`, {
    method: 'POST',
    headers: { Authorization: `Bearer ${token}` },
  });
  const text = await res.text();
  if (!res.ok) throw new Error(`introspect ${res.status}: ${text}`);
  return JSON.parse(text);
}

/**
 * Prefer pre-installed deps from scripts/node_modules (run `npm install` in scripts/ once).
 * Falls back to a temp install only when the local package is absent, so CI environments
 * can pre-install dependencies and avoid live npm downloads on every test run.
 */
async function useOrInstallSdk() {
  const localModules = join(__dirname, 'node_modules', '@pr402', 'subscription-seller');
  if (existsSync(localModules)) {
    // Fast path: use already-installed local deps
    return { dir: __dirname, runnerPath: join(__dirname, '_e2e_runner.mjs'), cleanup: () => {} };
  }

  // Slow path: temp install (first-time or CI without pre-install)
  console.log('Local node_modules not found; installing to temp dir (run `npm install` in scripts/ to avoid this)...');
  const dir = mkdtempSync(join(tmpdir(), 'sub-seller-e2e-'));
  writeFileSync(join(dir, 'package.json'), JSON.stringify({ type: 'module', private: true }, null, 2));
  try {
    execSync('npm install @pr402/subscription-seller@0.1.0 tweetnacl@1.0.3', {
      cwd: dir,
      stdio: ['pipe', 'pipe', 'inherit'], // show stderr so install errors are visible
    });
  } catch (err) {
    rmSync(dir, { recursive: true, force: true });
    throw new Error(`npm install failed: ${err.message}`);
  }
  return {
    dir,
    runnerPath: join(dir, '_e2e_runner.mjs'),
    cleanup: () => rmSync(dir, { recursive: true, force: true }),
  };
}

async function main() {
  const wallet = walletFromKeypair(keypairPath);
  const signer = loadSigner(keypairPath);
  const secretKey = Uint8Array.from(JSON.parse(readFileSync(keypairPath, 'utf8')));

  console.log(`auth-only E2E: ${baseUrl} service_id=${serviceId} wallet=${wallet}`);

  if (dryRun) {
    console.log('dry-run: would issue → verify → introspect → revoke → revocations');
    return;
  }

  if (revokeOnly) {
    if (!jtiArg || jtiArg === true) {
      console.error('--revoke-only requires --jti <uuid>');
      process.exit(2);
    }
    await revokeToken(wallet, signer, jtiArg);
    console.log('OK: revoked', jtiArg);
    return;
  }

  const { dir, runnerPath, cleanup } = await useOrInstallSdk();

  // Write the inline runner script to the resolved dir
  writeFileSync(runnerPath, `
import {
  issueTokenViaAuthService,
  verifyTokenWithJwks,
  createRevocationPollCache,
} from '@pr402/subscription-seller';
import { readFileSync } from 'node:fs';

const input = JSON.parse(readFileSync(0, 'utf8'));
const { baseUrl, serviceId, expectedIss, payer, wallet, secretKeyB64 } = input;

const secretKey = Buffer.from(secretKeyB64, 'base64');
async function signMessage(message) {
  const nacl = await import('tweetnacl');
  const sig = nacl.default.sign.detached(message, secretKey);
  return Buffer.from(sig).toString('base64');
}

const issued = await issueTokenViaAuthService({
  baseUrl,
  merchantWallet: wallet,
  serviceId,
  payer,
  tier: 'hourly',
  resources: ['*'],
  signMessage,
});

const payload = await verifyTokenWithJwks(issued.token, {
  jwksUrl: baseUrl + '/.well-known/jwks.json',
  expectedIss,
  expectedSub: serviceId,
});

const cache = createRevocationPollCache({ baseUrl, serviceId, intervalSec: 60 });
await cache.pollOnce();

console.log(JSON.stringify({ token: issued.token, jti: issued.jti, payer: payload.payer, tier: payload.tier }));
`);

  try {
    const input = JSON.stringify({
      baseUrl, serviceId, expectedIss, payer, wallet,
      secretKeyB64: Buffer.from(secretKey).toString('base64'),
    });
    const out = execSync(`node ${runnerPath}`, { cwd: dir, input, encoding: 'utf8' });
    const { token, jti, payer: gotPayer, tier } = JSON.parse(out.trim().split('\n').pop());

    if (gotPayer !== payer) throw new Error(`payer mismatch: ${gotPayer} !== ${payer}`);
    if (tier !== 'hourly') throw new Error(`tier mismatch: ${tier}`);

    const intro1 = await introspect(token);
    if (!intro1.active) throw new Error('expected active token after issue');
    console.log('OK: issue + verifyTokenWithJwks + introspect active');

    await revokeToken(wallet, signer, jti);
    const intro2 = await introspect(token);
    if (intro2.active) throw new Error('expected inactive after revoke');
    console.log('OK: revoke + introspect inactive');
    console.log('auth-only E2E passed');
  } finally {
    cleanup();
  }
}

main().catch((err) => {
  console.error(err.message || err);
  process.exit(1);
});
