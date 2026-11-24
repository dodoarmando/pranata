// Lighthouse CI against the running e2e stack (`just perf` brings it up).
// Lighthouse's default mobile profile (simulated slow-4G, 4x CPU slowdown),
// with the form factor spelled out so a changed default cannot move the gate.
const { chromium } = require("@playwright/test");

const origin = `https://localhost:${process.env.PRANATA_E2E_PORT ?? "8443"}`;

module.exports = {
	ci: {
		collect: {
			url: [`${origin}/`],
			numberOfRuns: 3,
			// Playwright's Chromium, so CI needs no separate Chrome install.
			chromePath: process.env.CHROME_PATH || chromium.executablePath(),
			settings: {
				formFactor: "mobile",
				screenEmulation: {
					mobile: true,
					width: 412,
					height: 823,
					deviceScaleFactor: 1.75,
					disabled: false,
				},
				// The stack uses Caddy's internal CA.
				chromeFlags: "--no-sandbox --headless=new --ignore-certificate-errors",
				onlyCategories: ["performance", "accessibility"],
			},
		},
		assert: {
			assertions: {
				"categories:performance": ["error", { minScore: 0.9 }],
				"categories:accessibility": ["error", { minScore: 0.95 }],
			},
		},
		upload: { target: "filesystem", outputDir: "../../target/lighthouse" },
	},
};
