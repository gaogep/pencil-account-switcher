import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

test('disposable version edit preserves LF and Windows CRLF and rejects older versions', () => {
  const script = fileURLToPath(new URL('./set_rehearsal_version.mjs', import.meta.url));
  for (const newline of ['\n', '\r\n']) {
    const root = mkdtempSync(join(tmpdir(), 'switchboard-version-'));
    try {
      mkdirSync(join(root, 'src-tauri'));
      const cargo = '[package]\nname = "switchboard-app"\nversion = "2.0.0"\n'.replaceAll('\n', newline);
      const lock = '[[package]]\nname = "unrelated"\nversion = "2.0.0"\n\n[[package]]\nname = "switchboard-app"\nversion = "2.0.0"\n'.replaceAll('\n', newline);
      writeFileSync(join(root, 'src-tauri/Cargo.toml'), cargo);
      writeFileSync(join(root, 'Cargo.lock'), lock);
      writeFileSync(join(root, 'src-tauri/tauri.conf.json'), JSON.stringify({ version: '2.0.0' }));
      const run = version => spawnSync(process.execPath, [script, version], { cwd: root, env: { ...process.env, GITHUB_ACTIONS: 'true' }, encoding: 'utf8' });
      const rejected = run('1.0.1');
      assert.notEqual(rejected.status, 0);
      assert.equal(readFileSync(join(root, 'Cargo.lock'), 'utf8'), lock);
      const edited = run('2.0.1');
      assert.equal(edited.status, 0, edited.stderr);
      assert.equal(readFileSync(join(root, 'src-tauri/Cargo.toml'), 'utf8'), cargo.replace('version = "2.0.0"', 'version = "2.0.1"'));
      assert.equal(readFileSync(join(root, 'Cargo.lock'), 'utf8'), lock.replace(`name = "switchboard-app"${newline}version = "2.0.0"`, `name = "switchboard-app"${newline}version = "2.0.1"`));
      assert.equal(JSON.parse(readFileSync(join(root, 'src-tauri/tauri.conf.json'), 'utf8')).version, '2.0.1');
    } finally { rmSync(root, { recursive: true, force: true }); }
  }
});
