import assert from 'node:assert/strict';
import { createPublicKey } from 'node:crypto';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import { assetNames, checkVersion, verifyAssets } from './release_assets.mjs';
import { signFile, signingKey } from './sign_update.mjs';

test('release assets retain legacy mac naming, reject wrong tags, missing signatures and modified bytes', () => {
  const version = JSON.parse(readFileSync('src-tauri/tauri.conf.json', 'utf8')).version;
  assert.equal(checkVersion(`v${version}`), version);
  assert.throws(() => checkVersion('v99.0.0'), /does not match/);
  const names = assetNames(version);
  assert.match(names.mac, /-arm64\.zip$/);
  assert.match(names.windows, /-windows-x64-setup\.exe$/);
  assert.ok(!names.windows.includes('arm64') && !names.windows.includes('.zip'));
  const root = mkdtempSync(join(tmpdir(), 'switchboard-assets-'));
  try {
    const key = signingKey(Buffer.alloc(32, 37).toString('base64'));
    const publicKey = createPublicKey(key).export({ format: 'der', type: 'spki' }).subarray(-32).toString('base64');
    for (const name of Object.values(names)) writeFileSync(join(root, name), `fixture for ${name}`);
    signFile(join(root, names.mac), key);
    assert.throws(() => verifyAssets(root, version, publicKey), /missing or unexpected/);
    signFile(join(root, names.windows), key);
    verifyAssets(root, version, publicKey);
    writeFileSync(join(root, names.windows), 'tampered');
    assert.throws(() => verifyAssets(root, version, publicKey), /Invalid update signature/);
  } finally { rmSync(root, { recursive: true, force: true }); }
});
