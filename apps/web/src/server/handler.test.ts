import { describe, expect, it } from "@rstest/core";
import { NONCE_PLACEHOLDER } from "./csp";
import { createHandler } from "./handler";
import { createHtmlCache } from "./html-cache";

const TOKEN = "s3cret-purge-token";

function setup(render?: (req: Request) => Response) {
	let renders = 0;
	const cache = createHtmlCache({ max: 10, ttlMs: 60_000 });
	const handler = createHandler({
		cache,
		purgeToken: TOKEN,
		render: async (req) => {
			renders += 1;
			return (
				render?.(req) ??
				new Response(`<script nonce="${NONCE_PLACEHOLDER}">1</script>`, {
					headers: { "content-type": "text/html; charset=utf-8" },
				})
			);
		},
	});
	return { handler, cache, renders: () => renders };
}

const get = (path: string, headers?: Record<string, string>) =>
	new Request(`http://localhost${path}`, { headers });

describe("server handler", () => {
	it("serves MISS then HIT with a fresh nonce each time", async () => {
		const { handler, renders } = setup();
		const first = await handler(get("/?a=1"));
		const second = await handler(get("/?a=1"));
		expect(first.headers.get("x-cache")).toBe("MISS");
		expect(second.headers.get("x-cache")).toBe("HIT");
		expect(renders()).toBe(1);
		const n1 = /nonce-([^']+)'/.exec(
			first.headers.get("content-security-policy") ?? "",
		)?.[1];
		const n2 = /nonce-([^']+)'/.exec(
			second.headers.get("content-security-policy") ?? "",
		)?.[1];
		expect(n1).not.toBe(n2);
		expect(await first.text()).toContain(`nonce="${n1}"`);
		expect(await second.text()).toContain(`nonce="${n2}"`);
	});

	it("keys on path and query", async () => {
		const { handler, renders } = setup();
		await handler(get("/?a=1"));
		await handler(get("/?a=2"));
		expect(renders()).toBe(2);
	});

	it("bypasses admin, auth, internal paths, sessions and non-GET", async () => {
		const { handler, cache } = setup();
		for (const path of ["/admin", "/admin/x", "/masuk", "/akses/abc"]) {
			const res = await handler(get(path));
			expect(res.headers.get("x-cache")).toBe("BYPASS");
			expect(res.headers.get("content-security-policy")).toContain(
				"frame-ancestors 'none'",
			);
			expect(await res.text()).not.toContain(NONCE_PLACEHOLDER);
		}
		const withSession = await handler(
			get("/", { cookie: "a=b; pranata_session=xyz" }),
		);
		expect(withSession.headers.get("x-cache")).toBe("BYPASS");
		const post = await handler(
			new Request("http://localhost/", { method: "POST" }),
		);
		expect(post.headers.get("x-cache")).toBe("BYPASS");
		expect(cache.size()).toBe(0);
	});

	it("does not cache non-200 responses", async () => {
		const { handler, cache } = setup(
			() =>
				new Response("Situs desa belum disiapkan.", {
					status: 503,
					headers: { "content-type": "text/html" },
				}),
		);
		const res = await handler(get("/"));
		expect(res.status).toBe(503);
		expect(res.headers.get("x-cache")).toBe("MISS");
		expect(cache.size()).toBe(0);
	});

	it("purges with a valid token and hides the route otherwise", async () => {
		const { handler, cache } = setup();
		await handler(get("/"));
		expect(cache.size()).toBe(1);
		const purge = (headers?: Record<string, string>, method = "POST") =>
			handler(
				new Request("http://localhost/_internal/cache/purge", {
					method,
					headers,
				}),
			);
		expect((await purge({ "x-pranata-purge-token": "wrong" })).status).toBe(
			404,
		);
		expect((await purge()).status).toBe(404);
		expect(
			(await purge({ "x-pranata-purge-token": TOKEN }, "GET")).status,
		).toBe(404);
		expect(cache.size()).toBe(1);
		expect((await purge({ "x-pranata-purge-token": TOKEN })).status).toBe(204);
		expect(cache.size()).toBe(0);
	});

	it("rejects every purge when no token is configured", async () => {
		const handler = createHandler({
			cache: createHtmlCache({}),
			purgeToken: "",
			render: async () => new Response("x"),
		});
		const res = await handler(
			new Request("http://localhost/_internal/cache/purge", {
				method: "POST",
				headers: { "x-pranata-purge-token": "" },
			}),
		);
		expect(res.status).toBe(404);
	});
});
