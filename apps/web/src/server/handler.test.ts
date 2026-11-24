import { describe, expect, it } from "@rstest/core";
import { NONCE_PLACEHOLDER } from "@/lib/nonce";
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

	it("bypasses case and encoding variants of private paths", async () => {
		const { handler, cache, renders } = setup();
		const paths = [
			"/Admin",
			"/ADMIN/users",
			"/MASUK",
			"/%61dmin",
			"/%41dmin/x",
			"/akses%2Fabc",
			"/AKSES",
			"//admin",
			"/admin//x",
			"/./admin",
			"/%5Finternal/x",
		];
		for (const path of paths) {
			const res = await handler(get(path));
			expect(res.headers.get("x-cache") ?? "404").not.toMatch(/HIT|MISS/);
		}
		expect(cache.size()).toBe(0);
		expect(renders()).toBeGreaterThan(0);
	});

	it("bypasses malformed percent-encoding", async () => {
		const { handler, cache } = setup();
		const res = await handler(get("/%E0%A4%A"));
		expect(res.headers.get("x-cache")).toBe("BYPASS");
		expect(cache.size()).toBe(0);
	});

	it("still caches ordinary paths that merely start with the letters", async () => {
		const { handler, cache } = setup();
		await handler(get("/administrasi"));
		expect(cache.size()).toBe(1);
	});

	it("does not cache responses that set cookies or are private", async () => {
		const variants: Record<string, string>[] = [
			{ "set-cookie": "a=b" },
			{ "cache-control": "private, max-age=0" },
			{ "cache-control": "no-store" },
			{ "cache-control": "No-Cache" },
		];
		for (const headers of variants) {
			const { handler, cache } = setup(
				() =>
					new Response("<p>x</p>", {
						headers: { "content-type": "text/html", ...headers },
					}),
			);
			const res = await handler(get("/"));
			expect(res.headers.get("x-cache")).toBe("MISS");
			expect(cache.size()).toBe(0);
		}
	});

	it("drops a render that started before a purge", async () => {
		let release: () => void = () => {};
		const gate = new Promise<void>((resolve) => {
			release = resolve;
		});
		const cache = createHtmlCache({});
		const handler = createHandler({
			cache,
			purgeToken: TOKEN,
			render: async () => {
				await gate;
				return new Response("<p>old</p>", {
					headers: { "content-type": "text/html" },
				});
			},
		});
		const pending = handler(get("/"));
		await handler(
			new Request("http://localhost/_internal/cache/purge", {
				method: "POST",
				headers: { "x-pranata-purge-token": TOKEN },
			}),
		);
		release();
		await pending;
		expect(cache.size()).toBe(0);
	});

	it("shares one render between concurrent misses", async () => {
		let renders = 0;
		const handler = createHandler({
			cache: createHtmlCache({}),
			purgeToken: TOKEN,
			render: async () => {
				renders += 1;
				await new Promise((r) => setTimeout(r, 20));
				return new Response(`<script nonce="${NONCE_PLACEHOLDER}"></script>`, {
					headers: { "content-type": "text/html" },
				});
			},
		});
		const responses = await Promise.all(
			Array.from({ length: 25 }, () => handler(get("/"))),
		);
		expect(renders).toBe(1);
		const nonces = new Set(
			await Promise.all(
				responses.map(async (r) => /nonce="([^"]+)"/.exec(await r.text())?.[1]),
			),
		);
		expect(nonces.size).toBe(25);
	});

	it("does not share an uncacheable render between visitors", async () => {
		let renders = 0;
		const handler = createHandler({
			cache: createHtmlCache({}),
			purgeToken: TOKEN,
			render: async () => {
				renders += 1;
				await new Promise((r) => setTimeout(r, 10));
				return new Response("<p>x</p>", {
					headers: { "content-type": "text/html", "set-cookie": "a=b" },
				});
			},
		});
		await Promise.all([
			handler(get("/")),
			handler(get("/")),
			handler(get("/")),
		]);
		expect(renders).toBe(3);
	});
});
