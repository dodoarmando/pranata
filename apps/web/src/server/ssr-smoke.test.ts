import { execFile } from "node:child_process";
import { existsSync } from "node:fs";
import { createServer } from "node:http";
import type { AddressInfo } from "node:net";
import { resolve } from "node:path";
import { promisify } from "node:util";
import { afterAll, beforeAll, describe, expect, it } from "@rstest/core";

const run = promisify(execFile);
const serverEntry = resolve(process.cwd(), "dist/server/index.js");

const village = {
	code: "14.01.01.2001",
	name: "Desa Contoh",
	district: "X",
	regency: "Kabupaten Pelalawan",
	province: "Riau",
	time_zone: "Asia/Jakarta",
	overview_html: "<p>Selamat datang.</p>",
	hero: null,
};

// Runs the real built Start handler (through our entry) in a Bun child process.
const script = `
const entry = (await import(${JSON.stringify(serverEntry)})).default;
const res = await entry.fetch(new Request("http://localhost/"));
console.log(JSON.stringify({
	status: res.status,
	csp: res.headers.get("content-security-policy"),
	html: await res.text(),
}));
`;

describe("start ssr smoke", () => {
	const api = createServer((_req, res) => {
		res.setHeader("content-type", "application/json");
		res.end(JSON.stringify(village));
	});
	let apiUrl = "";

	beforeAll(async () => {
		if (!existsSync(serverEntry)) {
			throw new Error(
				"dist/server/index.js is missing: run `bun run build` before this test",
			);
		}
		await new Promise<void>((ok) => api.listen(0, "127.0.0.1", ok));
		apiUrl = `http://127.0.0.1:${(api.address() as AddressInfo).port}`;
	});
	afterAll(() => {
		api.close();
	});

	it("puts the response nonce on every inline script", async () => {
		const { stdout } = await run("bun", ["-e", script], {
			env: { ...process.env, PRANATA_API_INTERNAL_URL: apiUrl },
		});
		const out = JSON.parse(stdout.trim().split("\n").at(-1) ?? "{}") as {
			status: number;
			csp: string;
			html: string;
		};
		expect(out.status).toBe(200);
		expect(out.html).toContain("Desa");
		const n = /'nonce-([^']+)'/.exec(out.csp)?.[1];
		expect(n).toBeTruthy();
		expect(out.html).not.toContain("__PRANATA_NONCE__");

		const scripts = [...out.html.matchAll(/<script\b([^>]*)>/g)].map(
			(m) => m[1] ?? "",
		);
		const inline = scripts.filter((attrs) => !/\bsrc=/.test(attrs));
		expect(inline.length).toBeGreaterThan(0);
		for (const attrs of inline) {
			expect(attrs).toContain(`nonce="${n}"`);
		}
		// External scripts and the csp-nonce meta carry it too.
		for (const attrs of scripts.filter((a) => /\bsrc=/.test(a))) {
			expect(attrs).toContain(`nonce="${n}"`);
		}
		expect(out.html).toContain(`<meta property="csp-nonce" content="${n}"`);
	});
});
