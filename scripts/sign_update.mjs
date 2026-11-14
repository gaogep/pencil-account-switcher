import { createPrivateKey, createPublicKey, sign } from 'node:crypto';
import { readFileSync, writeFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';

export const UPDATE_PUBLIC_KEY = 'i4OQxXFQgGlNb+T2kwMmSawM6Ps+2YsU1uvtLsPGvpc=';

// Same 32-byte Ed25519 seed and base64 signature as Support/sign_update.swift.
export function signingKey(seedBase64) {
  const seed = Buffer.from(seedBase64 ?? '', 'base64');
  if (seed.length !== 32) throw new Error('UPDATE_SIGNING_KEY must be a base64-encoded 32-byte Ed25519 seed');
  return createPrivateKey({
    key: Buffer.concat([Buffer.from('302e020100300506032b657004220420', 'hex'), seed]),
    format: 'der', type: 'pkcs8',
  });
}
export function signFile(path, key) {
  const signature = sign(null, readFileSync(path), key).toString('base64');
  writeFileSync(`${path}.sig`, signature); // Deliberately no trailing newline (Swift format).
  return signature;
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    if (process.argv.length !== 3) throw new Error('Usage: UPDATE_SIGNING_KEY=... node scripts/sign_update.mjs <asset>');
    const key = signingKey(process.env.UPDATE_SIGNING_KEY);
    const rawPublicKey = createPublicKey(key).export({ format: 'der', type: 'spki' }).subarray(-32).toString('base64');
    if (rawPublicKey !== UPDATE_PUBLIC_KEY) throw new Error('Signing key does not match the existing update public key');
    signFile(process.argv[2], key);
    console.log(`${process.argv[2]}.sig`);
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
