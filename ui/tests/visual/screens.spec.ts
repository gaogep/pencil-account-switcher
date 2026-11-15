import { test, expect } from '@playwright/test';
import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { resolve, join } from 'node:path';
import { PNG } from 'pngjs';
import pixelmatch from 'pixelmatch';
import type { Snapshot } from '../../src/ipc';
const scenarios = readdirSync(resolve('public/scenarios')).filter(name => name.endsWith('.json')).map(name => name.slice(0, -5)).sort();
const platform = process.platform === 'win32' ? 'windows' : 'macos';
const baselineDir = resolve(`../Tests/baseline/${platform}`);
const reportDir = resolve(`visual-report/${platform}`);
mkdirSync(reportDir, { recursive: true });
const report = (name: string, html: string) => writeFileSync(join(reportDir, `${name}.html`), `<section>${html}</section>`);
const image = (bytes: Buffer) => `<img src="data:image/png;base64,${bytes.toString('base64')}" />`;
for (const name of scenarios) test(name, async ({ page }, info) => {
  const fixture: Snapshot = JSON.parse(readFileSync(resolve(`public/scenarios/${name}.json`), 'utf8'));
  const errors: string[] = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.goto(`/?mock=${name}`);
  await expect(page.locator('.panel')).toHaveAttribute('aria-busy', 'false');
  if (fixture.route !== 'main') await expect(page.getByRole('heading', { name: fixture.route === 'settings' ? '设置' : '管理账号', exact: true })).toBeVisible();
  if (fixture.confirmRemoveEmail) {
    const dialog = page.getByRole('alertdialog');
    await expect(dialog).toHaveText('从切号器移除此账号？' + `将删除 ${fixture.confirmRemoveEmail} 在本机的登录凭据与额度缓存；远端账号不受影响。` + `移除 ${fixture.confirmRemoveEmail}取消`);
    await expect(dialog.getByRole('button').nth(0)).toHaveText(`移除 ${fixture.confirmRemoveEmail}`);
    await expect(dialog.getByRole('button').nth(1)).toHaveText('取消');
  }
  await page.evaluate(() => document.fonts.ready);
  const panel = page.locator('.panel');
  for (const avatar of await panel.locator('.avatar').all()) {
    const size = await avatar.evaluate(el => el.closest('.current-account') ? 36 : 34);
    await expect(avatar).toHaveCSS('width', `${size}px`);
    await expect(avatar).toHaveCSS('height', `${size}px`);
    await expect(avatar).toHaveCSS('font-size', `${size === 36 ? 12 : 11}px`);
  }
  const bounds = await panel.boundingBox();
  expect(bounds?.width).toBe(410);
  expect(await page.locator('.screen-content').evaluate(el => el.scrollWidth <= el.clientWidth)).toBe(true);
  if (name === 'main-unregistered-live') await expect(page.getByRole('button', { name: '导入', exact: true })).toHaveCSS('color', 'rgb(39, 39, 42)');
  // Only the documented SF Symbols -> Lucide replacements and the repaired
  // import label are excluded. No text, rows, cards or dynamic layout is masked.
  const excluded = await panel.locator(name === 'main-unregistered-live' ? 'svg, .unregistered .link-button' : 'svg').evaluateAll(elements => elements.map(el => { const r = el.getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width, height: r.height }; }));
  const actual = await panel.screenshot({ animations: 'disabled', omitBackground: true });
  writeFileSync(join(reportDir, `${name}-actual.png`), actual);
  await info.attach('current', { body: actual, contentType: 'image/png' });
  expect(errors).toEqual([]);
  const basePath = join(baselineDir, `panel-${name}.png`);
  if (name === 'manage-confirm-remove') {
    report(name, `<h2>${name} (文案与按钮顺序)</h2>${existsSync(basePath) ? image(readFileSync(basePath)) : ''}${image(actual)}`);
    return;
  }
  if (!existsSync(basePath) && platform === 'windows') {
    report(name, `<h2>${name} (待人工确认后建立 Windows 基线)</h2>${image(actual)}`);
    return;
  }
  const expected = readFileSync(basePath); const a = PNG.sync.read(actual); const b = PNG.sync.read(expected);
  if (a.width !== b.width || a.height !== b.height) {
    report(name, `<h2>${name}: 尺寸不符 ${b.width}×${b.height} → ${a.width}×${a.height}</h2>${image(expected)}${image(actual)}`);
    expect({ width: a.width, height: a.height }, 'baseline dimensions must match exactly').toEqual({ width: b.width, height: b.height });
    return;
  }
  const diff = new PNG({ width: a.width, height: a.height });
  const changed = new Uint8Array(a.width * a.height);
  const raw = new PNG({ width: a.width, height: a.height });
  pixelmatch(b.data, a.data, raw.data, a.width, a.height, { threshold: 0.1, diffMask: true, includeAA: false });
  for (let i = 0; i < changed.length; i++) changed[i] = raw.data[i * 4 + 3] ? 1 : 0;
  let compared = changed.length;
  const ignored = new Set<number>();
  for (const rect of excluded) {
    for (let y = Math.max(0, Math.floor(rect.y * 2)); y < Math.min(a.height, Math.ceil((rect.y + rect.height) * 2)); y++) {
      for (let x = Math.max(0, Math.floor(rect.x * 2)); x < Math.min(a.width, Math.ceil((rect.x + rect.width) * 2)); x++) ignored.add(y * a.width + x);
    }
  }
  let count = 0;
  for (let i = 0; i < changed.length; i++) {
    if (ignored.has(i)) { compared--; continue; }
    if (changed[i]) { count++; diff.data.set([255, 0, 0, 255], i * 4); }
    else diff.data.set([a.data[i * 4], a.data[i * 4 + 1], a.data[i * 4 + 2], 80], i * 4);
  }
  const output = PNG.sync.write(diff); const ratio = count / compared;
  writeFileSync(join(reportDir, `${name}-diff.png`), output);
  await info.attach('difference', { body: output, contentType: 'image/png' });
  report(name, `<h2>${name}: ${(ratio * 100).toFixed(2)}% (阈值 1%)</h2>${image(expected)}${image(actual)}${image(output)}`);
  expect(ratio, 'pixel difference excluding documented exceptions').toBeLessThanOrEqual(0.01);
});
test.afterAll(() => {
  writeFileSync(join(reportDir, 'index.html'), `<!doctype html><meta charset="utf-8"><title>Switchboard 视觉对照</title><style>body{font-family:system-ui;padding:24px;background:#eee}section{display:flex;gap:16px;flex-wrap:wrap;margin-bottom:36px}h2{width:100%}img{width:410px;height:auto;object-fit:contain;align-self:flex-start}</style><h1>基线 / 新版 / 差异</h1>${readdirSync(reportDir).filter(name => name.endsWith('.html') && name !== 'index.html').sort().map(name => readFileSync(join(reportDir, name), 'utf8')).join('')}`);
});
