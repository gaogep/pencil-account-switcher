import { createPublicKey, verify } from 'node:crypto';
import { copyFileSync, mkdirSync, readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { execFileSync } from 'node:child_process';
import { pathToFileURL } from 'node:url';
import { UPDATE_PUBLIC_KEY } from './sign_update.mjs';

export function assetNames(version) {
  const prefix = `Pencil-Account-Switcher-${version}`;
  return { mac: `${prefix}-arm64.zip`, dmg: `${prefix}-arm64.dmg`, windows: `${prefix}-windows-x64-setup.exe` };
}
export function checkVersion(tag) {
  const version = JSON.parse(readFileSync('src-tauri/tauri.conf.json', 'utf8')).version;
  if (tag !== `v${version}`) throw new Error(`Release tag ${tag} does not match tauri.conf.json version ${version}`);
  const cargo = readFileSync('src-tauri/Cargo.toml', 'utf8').match(/^version = "([^"]+)"/m)?.[1];
  if (cargo !== version) throw new Error('Application Cargo.toml and tauri.conf.json versions differ');
  return version;
}
export function packageAssets(target, output) {
  const version = JSON.parse(readFileSync('src-tauri/tauri.conf.json', 'utf8')).version;
  const names = assetNames(version);
  const bundle = join('target', target, 'release', 'bundle');
  const unique = (folder, suffix) => {
    const items = readdirSync(folder).filter(name => name.endsWith(suffix));
    if (items.length !== 1) throw new Error(`Expected exactly one ${suffix} in ${folder}`);
    return join(folder, items[0]);
  };
  mkdirSync(output, { recursive: true });
  if (target === 'aarch64-apple-darwin') {
    const app = unique(join(bundle, 'macos'), '.app');
    const plist = join(app, 'Contents', 'Info.plist');
    const value = key => execFileSync('/usr/libexec/PlistBuddy', ['-c', `Print :${key}`, plist], { encoding: 'utf8' }).trim();
    if (value('CFBundleIdentifier') !== 'dev.switchboard.Switchboard' || value('CFBundleShortVersionString') !== version) throw new Error('Built app identity/version mismatch');
    execFileSync('/usr/bin/codesign', ['--verify', '--deep', '--strict', app]);
    execFileSync('/usr/bin/ditto', ['-c', '-k', '--keepParent', app, join(output, names.mac)]);
    copyFileSync(unique(join(bundle, 'dmg'), '.dmg'), join(output, names.dmg));
  } else if (target === 'x86_64-pc-windows-msvc') {
    copyFileSync(unique(join(bundle, 'nsis'), '-setup.exe'), join(output, names.windows));
  } else throw new Error(`Unsupported release target ${target}`);
}
export function verifyPackage(path, publicKeyBase64 = UPDATE_PUBLIC_KEY) {
  const key = createPublicKey({ key: Buffer.concat([Buffer.from('302a300506032b6570032100', 'hex'), Buffer.from(publicKeyBase64, 'base64')]), format: 'der', type: 'spki' });
  const signature = readFileSync(`${path}.sig`, 'utf8').trim();
  if (!/^[A-Za-z0-9+/]{86}==$/.test(signature)) throw new Error(`Invalid update signature encoding: ${path}`);
  if (!verify(null, readFileSync(path), key, Buffer.from(signature.trim(), 'base64'))) throw new Error(`Invalid update signature: ${path}`);
}
export function verifyAssets(directory, version, publicKeyBase64 = UPDATE_PUBLIC_KEY) {
  const names = assetNames(version);
  const expected = [names.mac, `${names.mac}.sig`, names.dmg, names.windows, `${names.windows}.sig`].sort();
  const actual = readdirSync(directory).sort();
  if (JSON.stringify(actual) !== JSON.stringify(expected)) throw new Error('Release asset names or signatures are missing or unexpected');
  for (const asset of [names.mac, names.windows]) verifyPackage(join(directory, asset), publicKeyBase64);
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    const [action, arg, extra] = process.argv.slice(2);
    if (action === 'check-version') console.log(checkVersion(arg));
    else if (action === 'verify-package') verifyPackage(arg);
    else if (action === 'package') packageAssets(arg, extra ?? 'build/release');
    else if (action === 'verify') verifyAssets(arg, checkVersion(extra));
    else throw new Error('Usage: release_assets.mjs check-version <tag> | verify-package <asset> | package <target> [output] | verify <directory> <tag>');
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
