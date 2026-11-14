import assert from 'node:assert/strict';
import { createPublicKey, verify } from 'node:crypto';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import test from 'node:test';
import { signFile, signingKey, UPDATE_PUBLIC_KEY } from './sign_update.mjs';

test('Ed25519 signature roundtrip, tampering, key shape and production-key guard', () => {
  const root = mkdtempSync(join(tmpdir(), 'switchboard-sign-'));
  try {
    const path = join(root, 'fixture.zip');
    writeFileSync(path, Buffer.from([0, 1, 2, 255]));
    const seed = Buffer.alloc(32, 17).toString('base64');
    const key = signingKey(seed);
    const signature = signFile(path, key);
    assert.equal(readFileSync(`${path}.sig`, 'utf8'), signature);
    assert.equal(signature.length, 88);
    assert.ok(verify(null, readFileSync(path), createPublicKey(key), Buffer.from(signature, 'base64')));
    assert.ok(!verify(null, Buffer.from('modified'), createPublicKey(key), Buffer.from(signature, 'base64')));
    assert.throws(() => signingKey('bad'), /32-byte/);
    const cli = spawnSync(process.execPath, ['scripts/sign_update.mjs', path], { env: { ...process.env, UPDATE_SIGNING_KEY: seed }, encoding: 'utf8' });
    assert.equal(cli.status, 1);
    assert.match(cli.stderr, /does not match/);
    // All clients and both bundle configurations must keep the original public key.
    for (const source of ['Support/Info.plist', 'src-tauri/Info.plist', 'crates/switchboard-core/src/update.rs']) {
      assert.ok(readFileSync(source, 'utf8').includes(UPDATE_PUBLIC_KEY));
    }
  } finally { rmSync(root, { recursive: true, force: true }); }
});

test('Node and legacy Swift signatures use the same encoding and verify with the same public key', { skip: process.platform !== 'darwin' }, () => {
  const root = mkdtempSync(join(tmpdir(), 'switchboard-sign-swift-'));
  try {
    const path = join(root, 'fixture.zip');
    writeFileSync(path, Buffer.from('legacy update signing compatibility\0binary'));
    const seed = Buffer.alloc(32, 29).toString('base64');
    const expected = signFile(path, signingKey(seed));
    const swift = spawnSync('swift', ['Support/sign_update.swift', path], { env: { ...process.env, UPDATE_SIGNING_KEY: seed }, encoding: 'utf8' });
    assert.equal(swift.status, 0, swift.stderr);
    const swiftSignature = readFileSync(`${path}.sig`, 'utf8');
    assert.match(swiftSignature, /^[A-Za-z0-9+/]{86}==$/);
    const publicKey = createPublicKey(signingKey(seed));
    assert.ok(verify(null, readFileSync(path), publicKey, Buffer.from(swiftSignature, 'base64')));
    // Current CryptoKit randomizes signatures. Compare wire format and verification,
    // rather than requiring identical bytes from independent signing operations.
    writeFileSync(`${path}.sig`, expected);
    const publicBase64 = publicKey.export({ format: 'der', type: 'spki' }).subarray(-32).toString('base64');
    const checked = spawnSync('swift', ['-e', `import CryptoKit; import Foundation
let path = CommandLine.arguments[1]
let key = try Curve25519.Signing.PublicKey(rawRepresentation: Data(base64Encoded: CommandLine.arguments[2])!)
let sig = Data(base64Encoded: try String(contentsOfFile: path + ".sig", encoding: .utf8))!
if !key.isValidSignature(sig, for: try Data(contentsOf: URL(fileURLWithPath: path))) { exit(1) }`, path, publicBase64], { encoding: 'utf8' });
    assert.equal(checked.status, 0, checked.stderr);
  } finally { rmSync(root, { recursive: true, force: true }); }
});
