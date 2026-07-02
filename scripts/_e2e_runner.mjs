
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
