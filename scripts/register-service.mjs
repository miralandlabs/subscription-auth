#!/usr/bin/env node
/**
 * Register a service on subscription-auth (wallet-signed).
 *
 * Usage:
 *   node scripts/register-service.mjs \
 *     --keypair ../../demo-wallets/seller-keypair.json \
 *     [--base-url https://preview.auth.ipay.sh] \
 *     [--service-id e2e.local.ipay.sh] \
 *     [--service-url http://127.0.0.1:3000] \
 *     [--dry-run]
 *
 * Loads scripts/.env.local if present (optional).
 */
import { readFileSync, existsSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
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
const serviceUrl = arg('service-url', process.env.SUBSCRIPTION_AUTH_SERVICE_URL) || 'http://127.0.0.1:3000';
const dryRun = arg('dry-run') === true;

/**
 * Parse --allowlist flag.
 * Accepts:
 *   --allowlist "*"                         → ["*"]
 *   --allowlist "/api/v1/data,/api/v1/echo" → ["/api/v1/data", "/api/v1/echo"]
 *   --allowlist '["*"]'                     → ["*"]  (JSON array string)
 * Defaults to ["*"] if not provided, but prints a warning encouraging explicit scoping.
 */
function parseAllowlist(raw) {
  if (!raw || raw === true) {
    console.warn(
      'Warning: --allowlist not specified; defaulting to ["*"] (all resources).\n' +
      '  Tip: pass --allowlist "/api/v1/data,/api/v1/echo" to restrict token scope.',
    );
    return ['*'];
  }
  // Try JSON array first
  if (raw.startsWith('[')) {
    try { return JSON.parse(raw); } catch {}
  }
  // Comma-separated paths
  return raw.split(',').map((s) => s.trim()).filter(Boolean);
}

const allowlist = parseAllowlist(arg('allowlist'));

if (!keypairPath || keypairPath === true) {
  console.error('Required: --keypair <path> or MERCHANT_KEYPAIR in scripts/.env.local');
  process.exit(2);
}

function loadSigner(path) {
  const raw = JSON.parse(readFileSync(path, 'utf8'));
  const bytes = Uint8Array.from(raw);
  const seed = Buffer.from(bytes.slice(0, 32));
  if (seed.length !== 32) throw new Error('keypair must contain at least 32 bytes');
  const pkcs8 = Buffer.concat([Buffer.from('302e020100300506032b657004220420', 'hex'), seed]);
  return createPrivateKey({ key: pkcs8, format: 'der', type: 'pkcs8' });
}

function walletFromKeypair(path) {
  const raw = Uint8Array.from(JSON.parse(readFileSync(path, 'utf8')));
  if (raw.length < 64) throw new Error('keypair must be 64 bytes');
  return base58Encode(raw.slice(32, 64));
}

async function fetchRetry(url, init, attempts = 5, delayMs = 2000) {
  let last;
  for (let i = 0; i < attempts; i += 1) {
    try {
      return await fetch(url, init);
    } catch (err) {
      last = err;
      if (i < attempts - 1) {
        await new Promise((resolve) => setTimeout(resolve, delayMs));
      }
    }
  }
  throw last;
}

async function main() {
  let wallet = arg('wallet');
  if (!wallet || wallet === true) {
    wallet = walletFromKeypair(keypairPath);
  }

  const allowlistJson = JSON.stringify(allowlist);

  const challengeUrl = new URL(`/v1/services/${wallet}/challenge`, baseUrl);
  challengeUrl.searchParams.set('action', 'register');
  challengeUrl.searchParams.set('service_id', serviceId);
  challengeUrl.searchParams.set('service_url', serviceUrl);
  challengeUrl.searchParams.set('resources_allowlist_json', allowlistJson);

  console.log(`baseUrl:     ${baseUrl}`);
  console.log(`wallet:      ${wallet}`);
  console.log(`service_id:  ${serviceId}`);
  console.log(`service_url: ${serviceUrl}`);
  console.log(`allowlist:   ${JSON.stringify(allowlist)}`);

  if (dryRun) {
    console.log('dry-run: would GET', challengeUrl.toString());
    return;
  }

  const chRes = await fetchRetry(challengeUrl.toString());
  const chText = await chRes.text();
  if (!chRes.ok) {
    throw new Error(`challenge failed ${chRes.status}: ${chText}`);
  }
  const { message } = JSON.parse(chText);
  if (!message) throw new Error('challenge missing message');

  const signer = loadSigner(keypairPath);
  const signature = edSign(null, Buffer.from(message, 'utf8'), signer).toString('base64');

  const registerUrl = `${baseUrl}/v1/services/${wallet}/register`;
  const body = {
    message,
    signature,
    service_id: serviceId,
    service_url: serviceUrl,
    resources_allowlist: allowlist,
  };

  const regRes = await fetchRetry(registerUrl, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
  const regText = await regRes.text();
  let regJson;
  try {
    regJson = JSON.parse(regText);
  } catch {
    regJson = { raw: regText };
  }

  if (
    regRes.status === 403 &&
    (String(regText).includes('already registered') ||
      regJson?.message === 'service_id already registered')
  ) {
    console.log('OK: service_id already registered (nothing to do — exit 0)');
    console.log(`  service_id:  ${serviceId}`);
    console.log(`  service_url: ${serviceUrl} (unchanged; auth keeps the URL from first register)`);
    console.log(
      '  To change service_url later, use subscription-auth action=update (not register).',
    );
    return;
  }
  if (!regRes.ok) {
    throw new Error(`register failed ${regRes.status}: ${regText}`);
  }

  console.log('OK: registered');
  console.log(JSON.stringify(regJson, null, 2));
}

main().catch((err) => {
  console.error(err.message || err);
  process.exit(1);
});
