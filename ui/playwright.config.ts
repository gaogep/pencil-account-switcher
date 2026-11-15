import { defineConfig } from '@playwright/test';
export default defineConfig({
  testDir: './tests/visual',
  globalSetup: './tests/visual/setup.ts',
  outputDir: './test-results',
  fullyParallel: false,
  workers: 1,
  retries: 0,
  reporter: [['list'], ['html', { open: 'never' }]],
  use: { browserName: 'webkit', viewport: { width: 410, height: 1000 }, deviceScaleFactor: 2, locale: 'zh-CN', timezoneId: 'Asia/Singapore', reducedMotion: 'reduce', baseURL: 'http://127.0.0.1:1420' },
  webServer: { command: 'npm run dev -- --host 127.0.0.1', url: 'http://127.0.0.1:1420', reuseExistingServer: !process.env.CI },
});
