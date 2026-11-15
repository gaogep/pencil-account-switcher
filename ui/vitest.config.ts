import { defineConfig } from 'vitest/config';
import preact from '@preact/preset-vite';
export default defineConfig({ plugins: [preact()], test: { environment: 'jsdom', setupFiles: ['./src/tests/setup.ts'], include: ['src/**/*.test.{ts,tsx}'] } });
