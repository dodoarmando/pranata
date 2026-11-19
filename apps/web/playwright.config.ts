import { defineConfig, devices } from '@playwright/test';

// Runs against the production images started by `just e2e`, never the dev server.
export default defineConfig({
  testDir: 'e2e',
  // `.e2e.ts`, not `.spec.ts`, so Rstest does not pick these up.
  testMatch: '**/*.e2e.ts',
  forbidOnly: !!process.env.CI,
  use: { baseURL: 'http://127.0.0.1:13000', trace: 'retain-on-failure' },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
});
