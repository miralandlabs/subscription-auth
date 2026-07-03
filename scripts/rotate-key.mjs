#!/usr/bin/env node
/**
 * rotate-key.mjs — JWKS key rotation helper for subscription-auth.
 *
 * Key rotation is a two-step process with a 24h overlap window:
 *   Step 1 (OLD key): add the OLD key's public JWK to the DB so it keeps
 *          being served in /.well-known/jwks.json after you switch to the new key.
 *   Step 2 (NEW key): update Vercel env vars with the new private key and kid,
 *          then deploy.
 *
 * This script handles Step 1: it reads a PEM private key and inserts its
 * public JWK into subscription_auth_signing_keys with retired_at = NOW().
 * The JWKS handler serves keys with retired_at in the last 24h, giving
 * existing tokens time to expire while the new key is live.
 *
 * Usage:
 *   # Add the OLD (retiring) key before switching to the new one:
 *   node scripts/rotate-key.mjs \
 *     --pem /path/to/old-private.pem \
 *     --kid auth-key-1
 *
 *   # --dry-run: print the SQL and JWK without writing to DB
 *   node scripts/rotate-key.mjs --pem ... --kid ... --dry-run
 *
 * Loads scripts/.env.local if present (for DATABASE_URL).
 */

import { readFileSync, existsSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createPublicKey } from 'node:crypto';

const __dirname = dirname(fileURLToPath(import.meta.url));

// ─── load .env.local ──────────────────────────────────────────────────────────
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

const pemPath = arg('pem');
const kid     = arg('kid');
const dryRun  = arg('dry-run') === true;

if (!pemPath || pemPath === true || !kid || kid === true) {
  console.error('Usage: node scripts/rotate-key.mjs --pem <path/to/private.pem> --kid <key-id> [--dry-run]');
  process.exit(2);
}

// ─── derive public JWK from PEM ───────────────────────────────────────────────
function publicJwkFromPem(pemPath, kid) {
  const pem = readFileSync(pemPath, 'utf8');
  const pub = createPublicKey(pem);
  const jwk = pub.export({ format: 'jwk' });
  return {
    kty: 'RSA',
    kid,
    use: 'sig',
    alg: 'RS256',
    n: jwk.n,
    e: jwk.e,
  };
}

async function main() {
  console.log(`Generating public JWK from ${pemPath} (kid=${kid}) …`);
  const jwk = publicJwkFromPem(pemPath, kid);
  const jwkJson = JSON.stringify(jwk);

  const sql = `
-- Insert retiring key into JWKS rotation table.
-- Run this BEFORE updating SUBSCRIPTION_AUTH_RSA_PRIVATE_KEY_PEM in Vercel.
-- The JWKS handler serves keys with retired_at within the last 24h, so existing
-- tokens signed by this key will keep verifying during the overlap window.
INSERT INTO subscription_auth_signing_keys (kid, public_jwk, active_from, retired_at)
VALUES (
  '${kid}',
  '${jwkJson.replace(/'/g, "''")}'::jsonb,
  NOW() - INTERVAL '1 second',  -- mark as already active (it has been)
  NOW()                          -- retire immediately
)
ON CONFLICT (kid) DO UPDATE
  SET public_jwk  = EXCLUDED.public_jwk,
      retired_at  = NOW();
`.trim();

  console.log('\n── Public JWK ──────────────────────────────────────');
  console.log(JSON.stringify(jwk, null, 2));
  console.log('\n── SQL to run BEFORE switching to the new key ──────');
  console.log(sql);

  if (dryRun) {
    console.log('\n[dry-run] Nothing written to the database.');
    return;
  }

  const dbUrl = process.env.DATABASE_URL;
  if (!dbUrl) {
    console.error('\nERROR: DATABASE_URL not set. Set it in scripts/.env.local or pass as env var.');
    console.error('Or copy the SQL above and run it manually:');
    console.error('  psql "$DATABASE_URL" -c "..."');
    process.exit(1);
  }

  // Dynamically import pg (only needed if actually writing to DB)
  let pg;
  try {
    pg = await import('pg');
  } catch {
    console.error('\nERROR: "pg" package not found. Install it:');
    console.error('  npm install pg   (in the scripts/ directory)');
    console.error('\nOr copy the SQL above and run it manually via psql.');
    process.exit(1);
  }

  const client = new pg.default.Client({ connectionString: dbUrl });
  await client.connect();
  try {
    await client.query(sql);
    console.log('\n✅ Key inserted into subscription_auth_signing_keys.');
    console.log('Next steps:');
    console.log('  1. Update SUBSCRIPTION_AUTH_RSA_PRIVATE_KEY_PEM in Vercel with the new private key.');
    console.log(`  2. Update SUBSCRIPTION_AUTH_KEY_ID to a new value (e.g. "${kid}-next").`);
    console.log('  3. Deploy — JWKS will serve both keys for 24h while old tokens expire.');
  } finally {
    await client.end();
  }
}

main().catch((err) => {
  console.error(err.message || err);
  process.exit(1);
});
