import https from "node:https";
import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";
import {
	NOINIT_ORIGIN,
	ORIGIN,
	OVERVIEW_TEXT,
	stackScript,
	VILLAGE_HEADING,
} from "./stack";

// D-06: the public site is built for a phone first, so every test here runs
// at 390x844.
const PHONE = { width: 390, height: 844 };
test.use({ viewport: PHONE });

const AXE_TAGS = ["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"];
// size-limit counts 1 KB as 1000 bytes; use the same unit for both gates.
const JS_BUDGET_BYTES = 120_000;

// Bytes on the wire for a gzip-capable client (the budget is stated in gzip).
function gzipBytes(url: string): Promise<number> {
	return new Promise((resolve, reject) => {
		https
			.get(
				url,
				{ rejectUnauthorized: false, headers: { "accept-encoding": "gzip" } },
				(res) => {
					if (res.headers["content-encoding"] !== "gzip") {
						reject(new Error(`${url} was not gzip encoded`));
						return;
					}
					let size = 0;
					res.on("data", (chunk: Buffer) => {
						size += chunk.length;
					});
					res.on("end", () => resolve(size));
				},
			)
			.on("error", reject);
	});
}

function nonceOf(csp: string | undefined): string {
	const match = /script-src [^;]*'nonce-([^']+)'/.exec(csp ?? "");
	if (!match?.[1]) throw new Error(`no nonce in CSP: ${csp}`);
	return match[1];
}

test("renders content without javascript", async ({ browser }) => {
	const context = await browser.newContext({
		javaScriptEnabled: false,
		ignoreHTTPSErrors: true,
		viewport: PHONE,
	});
	const page = await context.newPage();
	await page.goto(ORIGIN);
	await expect(page.getByRole("heading", { level: 1 })).toHaveText(
		VILLAGE_HEADING,
	);
	await expect(page.getByText(OVERVIEW_TEXT)).toBeVisible();
	await context.close();
});

test("makes no third-party requests", async ({ page }) => {
	const urls: string[] = [];
	page.on("request", (req) => urls.push(req.url()));
	await page.goto(ORIGIN, { waitUntil: "networkidle" });
	const foreign = urls.filter(
		(url) => !url.startsWith("data:") && new URL(url).origin !== ORIGIN,
	);
	expect(foreign).toEqual([]);
	expect(urls.length).toBeGreaterThan(1);
});

test("has no axe violations", async ({ page }) => {
	await page.goto(ORIGIN, { waitUntil: "networkidle" });
	const result = await new AxeBuilder({ page }).withTags(AXE_TAGS).analyze();
	expect(result.violations).toEqual([]);
});

test("sends csp with a fresh nonce per response", async ({ request }) => {
	const first = await request.get(ORIGIN);
	const second = await request.get(ORIGIN);
	const a = nonceOf(first.headers()["content-security-policy"]);
	const b = nonceOf(second.headers()["content-security-policy"]);
	expect(a).not.toBe(b);
	expect(await first.text()).toContain(`nonce="${a}"`);
	expect(await second.text()).toContain(`nonce="${b}"`);
});

test("public js budget", async ({ page }) => {
	const scripts = new Set<string>();
	page.on("response", (res) => {
		if (res.request().resourceType() === "script") scripts.add(res.url());
	});
	await page.goto(ORIGIN, { waitUntil: "networkidle" });
	expect(scripts.size).toBeGreaterThan(0);
	let total = 0;
	for (const url of scripts) total += await gzipBytes(url);
	console.log(`public js: ${total} bytes gzip in ${scripts.size} files`);
	expect(total).toBeLessThanOrEqual(JS_BUDGET_BYTES);
});

test.describe("before the village exists", () => {
	test.beforeAll(() => {
		test.setTimeout(240_000);
		stackScript("up", true);
	});
	test.afterAll(() => {
		stackScript("down", true);
	});

	test("shows unprepared message before init", async ({ request }) => {
		const res = await request.get(NOINIT_ORIGIN);
		expect(res.status()).toBe(503);
		expect(await res.text()).toContain("Situs desa belum disiapkan.");
	});
});
