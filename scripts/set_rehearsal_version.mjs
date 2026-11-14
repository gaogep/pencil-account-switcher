// Changes only the disposable CI checkout; release source versions stay at 2.0.0.
import { readFileSync, writeFileSync } from 'node:fs';
if (process.env.GITHUB_ACTIONS !== 'true') throw new Error('Run upgrade rehearsals in an isolated GitHub Actions checkout');
const next = process.argv[2];
const configPath = 'src-tauri/tauri.conf.json';
const config = JSON.parse(readFileSync(configPath, 'utf8'));
const current = config.version;
if (!/^\d+\.\d+\.\d+$/.test(next ?? '') || next.localeCompare(current, 'en', { numeric: true }) <= 0) throw new Error('Rehearsal version must be a newer numeric x.y.z version');
for (const [path, before, after] of [
  ['src-tauri/Cargo.toml', `version = "${current}"`, `version = "${next}"`],
  ['Cargo.lock', `name = "switchboard-app"\nversion = "${current}"`, `name = "switchboard-app"\nversion = "${next}"`],
]) {
  const text = readFileSync(path, 'utf8');
  const newline = text.includes('\r\n') ? '\r\n' : '\n';
  const search = before.replaceAll('\n', newline);
  if (!text.includes(search)) throw new Error(`Unexpected application version in ${path}`);
  writeFileSync(path, text.replace(search, after.replaceAll('\n', newline)));
}
config.version = next;
writeFileSync(configPath, `${JSON.stringify(config, null, 2)}\n`);
