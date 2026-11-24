import { defineConfig, devices } from "@playwright/test";
import { ORIGIN } from "./e2e/stack";

// The stack (infra/dev/stack.sh) must be running; `just e2e` manages it.
// One worker, files in name order: admin-profile.spec.ts writes the overview
// that public-home.spec.ts then reads.
export default defineConfig({
	testDir: "./e2e",
	workers: 1,
	fullyParallel: false,
	forbidOnly: Boolean(process.env.CI),
	retries: process.env.CI ? 1 : 0,
	reporter: process.env.CI ? [["list"], ["html", { open: "never" }]] : "list",
	timeout: 60_000,
	expect: { timeout: 10_000 },
	use: {
		baseURL: ORIGIN,
		// Caddy's internal CA is not trusted by the browser.
		ignoreHTTPSErrors: true,
		locale: "id-ID",
		trace: "retain-on-failure",
	},
	projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
});
