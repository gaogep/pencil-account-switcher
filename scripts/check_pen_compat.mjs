#!/usr/bin/env node
import * as asar from '@electron/asar';

const archive = process.argv[2];
if (!archive || process.argv.length !== 3) {
  console.error('Usage: node scripts/check_pen_compat.mjs <app.asar>');
  process.exit(1);
}

try {
  const read = (file) => asar.extractFile(archive, file).toString('utf8');
  const version = JSON.parse(read('package.json')).version;
  console.log(`Pen version: ${version}`);

  const files = {
    'out/constants.js': read('out/constants.js'),
    'out/desktop-app-host.js': read('out/desktop-app-host.js'),
    'out/app-window.js': read('out/app-window.js'),
    'out/main.js': read('out/main.js'),
  };
  const checks = [
    ['CONFIG_FOLDER', 'out/constants.js', /(?:exports\.)?CONFIG_FOLDER\s*=\s*(?:node_)?path(?:_\d+\.default)?\.join\(\s*(?:node_)?os(?:_\d+\.default)?\.homedir\(\)\s*,\s*["']\.pencil["']\s*\)/],
    ['session filename', 'out/desktop-app-host.js', /`session-desktop\$\{(?:constants_\d+\.)?IS_DEV\s*\?\s*["']-dev["']\s*:\s*["']["']\}\.json`/],
    ['device ID', 'out/desktop-app-host.js', /function\s+getDesktopDeviceId\s*\([^)]*\)\s*\{\s*(?:const|let)\s+(\w+)\s*=\s*os\.hostname\(\)\s*\+\s*os\.platform\(\)\s*\+\s*os\.arch\(\)\s*;\s*return\s+crypto\.createHash\(["']md5["']\)\.update\(\1\)\.digest\(["']hex["']\)/],
    ['clientHost', 'out/desktop-app-host.js', /clientHost\s*:\s*os\.platform\(\)/],
    ['non-macOS close quits', 'out/app-window.js', /window\.on\(["']close["']\s*,(?:(?!window\.on\(["']closed["'])[\s\S])*?if\s*\(\s*!(?:constants_\d+\.)?IS_MAC\s*\)\s*\{\s*(?:electron_\d+\.)?app\.quit\(\)/],
    ['single instance lock', 'out/main.js', /requestSingleInstanceLock\(\)/],
  ];
  let failed = false;
  for (const [label, file, pattern] of checks) {
    if (pattern.test(files[file])) {
      console.log(`PASS ${label} (${file})`);
    } else {
      failed = true;
      const terms = { CONFIG_FOLDER: 'CONFIG_FOLDER =', 'session filename': 'session-desktop', 'device ID': 'getDesktopDeviceId', clientHost: 'clientHost', 'non-macOS close quits': 'window.on("close"', 'single instance lock': 'requestSingleInstanceLock' };
      const index = files[file].indexOf(terms[label]);
      const excerpt = index < 0 ? '(matching code not found)' : files[file].slice(Math.max(0, index - 100), index + 600);
      console.error(`FAIL ${label} (${file}):\n${excerpt}`);
    }
  }

  const jsFiles = asar.listPackage(archive)
    .map((file) => file.replace(/^[/\\]+/, '').replaceAll('\\', '/'))
    .filter((file) => /^out\/[^/]+\.js$/.test(file));
  let trayFound = false;
  for (const file of jsFiles) {
    const source = read(file);
    const match = /new\s+electron_1\.Tray\s*\(/.exec(source);
    if (match) {
      failed = true;
      trayFound = true;
      console.error(`FAIL Pen tray (${file}):\n${source.slice(Math.max(0, match.index - 100), match.index + 200)}`);
    }
  }
  if (!jsFiles.length) {
    failed = true;
    console.error('FAIL Pen tray: no out/*.js files found');
  } else if (!trayFound) {
    console.log('PASS Pen tray (no electron_1.Tray in out/*.js)');
  }
  if (failed) process.exitCode = 1;
} catch (error) {
  console.error(`Unable to inspect ${archive}: ${error.message}`);
  process.exitCode = 1;
}
