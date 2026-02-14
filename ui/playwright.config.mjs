import { defineConfig } from '@playwright/test';

const baseURL = process.env.UI_BASE_URL || 'http://127.0.0.1:17835';

export default defineConfig({
  testDir: './tests/e2e',
  timeout: 30_000,
  expect: {
    timeout: 10_000,
  },
  retries: 1,
  reporter: [['list']],
  use: {
    baseURL,
    headless: true,
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
  },
  webServer: process.env.UI_BASE_URL
    ? undefined
    : {
        command: 'node tests/e2e/mock-server.mjs',
        port: Number(process.env.UI_MOCK_PORT || 17835),
        timeout: 60_000,
        reuseExistingServer: !process.env.CI,
      },
});
