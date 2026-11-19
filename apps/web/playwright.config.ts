import { defineConfig, devices } from '@playwright/test';

// Runs against the production images started by `just e2e`, never the dev server.
export default defineConfig({
  testDir: 'e2e',
  // `.e2e.ts`, not `.spec.ts`, so Rstest does not pick these up.
  testMatch: '**/*.e2e.ts',
  forbidOnly: !!process.env.CI,
  // `localhost`, the API's public origin: passkeys cannot be bound to an IP address.
  use: { baseURL: 'http://localhost:13000', trace: 'retain-on-failure' },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
});
