#!/usr/bin/env node
/**
 * Read-only spike: check if a wallet meets a USD hold threshold in an allowlisted mint.
 *
 * Not a pr402 rail — exploration for yield-qualified subscription (seller-side entitlement).
 * Zero npm deps — uses Solana JSON-RPC + Jupiter quote API.
 *
 * Usage:
 *   node subscription-auth/scripts/check-hold-qualification.mjs \
 *     --wallet buyA5hR1Z9KtHQRBTmLkjsFfjAabDwdZtrRC6edqxAJ \
 *     --min-usd 300 \
 *     [--mint A1KLoBrKBde8Ty9qtNQUtq3C2ortoC3u7twggz7sEto6] \
 *     [--rpc https://api.devnet.solana.com]
 */
const USDC_MINT = 'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v';
const DEFAULT_USDY = 'A1KLoBrKBde8Ty9qtNQUtq3C2ortoC3u7twggz7sEto6';
const DEFAULT_RPC = 'https://api.devnet.solana.com';

function arg(name, fallback) {
  const i = process.argv.indexOf(`--${name}`);
  if (i === -1) return fallback;
  const v = process.argv[i + 1];
  return v && !v.startsWith('--') ? v : fallback;
}

async function fetchWithRetry(url, init, attempts = 5, delayMs = 2000) {
  let last;
  for (let i = 0; i < attempts; i += 1) {
    try {
      return await fetch(url, init);
    } catch (err) {
      last = err;
      if (i < attempts - 1) await new Promise((r) => setTimeout(r, delayMs));
    }
  }
  throw last;
}

async function rpc(rpcUrl, method, params) {
  const res = await fetchWithRetry(rpcUrl, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }),
  });
  const data = await res.json();
  if (data.error) {
    throw new Error(`RPC ${method}: ${data.error.message ?? JSON.stringify(data.error)}`);
  }
  return data.result;
}

async function tokenAmountRaw(rpcUrl, owner, mintStr) {
  const result = await rpc(rpcUrl, 'getTokenAccountsByOwner', [
    owner,
    { mint: mintStr },
    { encoding: 'jsonParsed' },
  ]);
  let total = 0n;
  for (const { account } of result?.value ?? []) {
    const amount = account?.data?.parsed?.info?.tokenAmount?.amount;
    if (amount) total += BigInt(amount);
  }
  return total;
}

async function usdValueViaJupiter(inputMint, amountRaw) {
  if (amountRaw === 0n) return 0;
  const url =
    `https://quote-api.jup.ag/v6/quote?inputMint=${inputMint}` +
    `&outputMint=${USDC_MINT}&amount=${amountRaw.toString()}&slippageBps=50`;
  const res = await fetchWithRetry(url);
  if (!res.ok) {
    const text = await res.text();
    throw new Error(`Jupiter quote ${res.status}: ${text.slice(0, 200)}`);
  }
  const data = await res.json();
  const out = BigInt(data.outAmount ?? '0');
  return Number(out) / 1_000_000;
}

async function main() {
  const wallet = arg('wallet');
  const minUsd = parseFloat(arg('min-usd', '0'));
  const mintStr = arg('mint', DEFAULT_USDY);
  const rpcUrl = arg('rpc', DEFAULT_RPC);

  if (!wallet) {
    console.error('Required: --wallet <pubkey>');
    process.exit(2);
  }
  if (!Number.isFinite(minUsd) || minUsd <= 0) {
    console.error('Required: --min-usd <positive number>');
    process.exit(2);
  }

  const amountRaw = await tokenAmountRaw(rpcUrl, wallet, mintStr);
  const decimals = 6;
  const tokenUi = Number(amountRaw) / 10 ** decimals;
  const usdValue = await usdValueViaJupiter(mintStr, amountRaw);
  const qualified = usdValue >= minUsd;

  console.log(JSON.stringify({
    wallet,
    mint: mintStr,
    tokenAmount: tokenUi,
    usdValue: Math.round(usdValue * 100) / 100,
    minUsd,
    qualified,
    note: 'Read-only entitlement check — no pr402 settle. Seller would extend JWT if qualified.',
  }, null, 2));

  process.exit(qualified ? 0 : 1);
}

main().catch((err) => {
  console.error(err.message || err);
  process.exit(1);
});
