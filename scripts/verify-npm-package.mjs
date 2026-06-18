#!/usr/bin/env node
/**
 * Verify @pr402/subscription-seller@0.1.0 from npm (isolated temp dir — never touches starter).
 *
 * Usage: node scripts/verify-npm-package.mjs [--version 0.1.0]
 */
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { execSync } from 'node:child_process';

const version = process.argv.includes('--version')
  ? process.argv[process.argv.indexOf('--version') + 1]
  : '0.1.0';

const dir = mkdtempSync(join(tmpdir(), 'sub-seller-npm-'));
console.log(`temp dir: ${dir}`);

try {
  writeFileSync(
    join(dir, 'package.json'),
    JSON.stringify({ name: 'verify-sub-seller', type: 'module', private: true }, null, 2),
  );

  execSync(`npm install @pr402/subscription-seller@${version}`, {
    cwd: dir,
    stdio: 'inherit',
  });

  writeFileSync(
    join(dir, 'smoke.mjs'),
    `import {
  issueToken,
  verifyToken,
  issueTokenViaAuthService,
  verifyTokenWithJwks,
  createRevocationPollCache,
  fetchIssueChallenge,
} from '@pr402/subscription-seller';

const exports = {
  issueTokenViaAuthService,
  verifyTokenWithJwks,
  createRevocationPollCache,
  fetchIssueChallenge,
};
for (const [name, fn] of Object.entries(exports)) {
  if (typeof fn !== 'function') throw new Error(\`missing export: \${name}\`);
}

const secret = 'test-secret-at-least-32-bytes-long!!';
const token = issueToken({
  payer: '0xabc',
  tier: 'hourly',
  secret,
  jti: 'npm-smoke-jti',
  iat: Math.floor(Date.now() / 1000),
});
const payload = verifyToken(token, { secret });
if (payload.tier !== 'hourly') throw new Error('HS256 round-trip failed');

console.log('OK: npm install + Tier B exports + HS256 smoke');
`,
  );

  execSync('node smoke.mjs', { cwd: dir, stdio: 'inherit' });
} finally {
  rmSync(dir, { recursive: true, force: true });
}

console.log(`verify-npm-package passed (@pr402/subscription-seller@${version})`);
