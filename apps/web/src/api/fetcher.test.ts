import { afterEach, describe, expect, it, rs } from "@rstest/core";
import { getCsrfToken, setCsrfToken } from "@/lib/csrf";
import { ApiError, apiFetch } from "./fetcher";

function mockFetch(response: Response) {
	return rs.spyOn(globalThis, "fetch").mockResolvedValue(response);
}

// Tests run in Node: stub a browser-like `window` for the client cases.
function asBrowser() {
	Object.assign(globalThis, { window: {} });
}

afterEach(() => {
	rs.restoreAllMocks();
	setCsrfToken(null);
	delete (globalThis as { window?: unknown }).window;
	delete process.env.PRANATA_API_INTERNAL_URL;
});

describe("apiFetch", () => {
	it("sends credentials and the csrf token on unsafe requests", async () => {
		asBrowser();
		setCsrfToken("tok");
		const spy = mockFetch(Response.json({ ok: true }));
		await apiFetch("/api/admin/village/profile", { method: "PUT" });
		const init = spy.mock.calls[0]?.[1] as RequestInit;
		expect(init.credentials).toBe("include");
		expect(new Headers(init.headers).get("x-csrf-token")).toBe("tok");
	});

	it("omits the csrf token on GET", async () => {
		asBrowser();
		setCsrfToken("tok");
		const spy = mockFetch(Response.json({}));
		await apiFetch("/api/auth/me", { method: "GET" });
		const init = spy.mock.calls[0]?.[1] as RequestInit;
		expect(new Headers(init.headers).has("x-csrf-token")).toBe(false);
	});

	it("returns strings such as updated_at untouched", async () => {
		mockFetch(Response.json({ updated_at: "2026-11-24T10:00:00.123456Z" }));
		const body = await apiFetch<{ updated_at: string }>("/api/x");
		expect(body.updated_at).toBe("2026-11-24T10:00:00.123456Z");
	});

	it("throws a typed error carrying the problem code", async () => {
		mockFetch(
			new Response(
				JSON.stringify({
					type: "x",
					title: "Stale",
					status: 409,
					code: "stale_write",
				}),
				{
					status: 409,
					headers: { "content-type": "application/problem+json" },
				},
			),
		);
		const err = await apiFetch("/api/x").catch((e: unknown) => e);
		expect(err).toBeInstanceOf(ApiError);
		expect((err as ApiError).code).toBe("stale_write");
		expect((err as ApiError).status).toBe(409);
	});

	it("falls back to internal_error when the body is not json", async () => {
		mockFetch(new Response("boom", { status: 502 }));
		const err = (await apiFetch("/api/x").catch((e: unknown) => e)) as ApiError;
		expect(err.code).toBe("internal_error");
	});

	it("returns undefined for 204 responses", async () => {
		mockFetch(new Response(null, { status: 204 }));
		expect(
			await apiFetch("/api/auth/logout", { method: "POST" }),
		).toBeUndefined();
	});

	it("keeps the url relative in the browser", async () => {
		asBrowser();
		const spy = mockFetch(Response.json({}));
		await apiFetch("/api/healthz");
		expect(spy.mock.calls[0]?.[0]).toBe("/api/healthz");
	});

	it("prefixes the internal api origin on the server", async () => {
		process.env.PRANATA_API_INTERNAL_URL = "http://api.internal:9000";
		const spy = mockFetch(Response.json({}));
		await apiFetch("/api/healthz");
		expect(spy.mock.calls[0]?.[0]).toBe("http://api.internal:9000/api/healthz");
	});

	it("never stores a csrf token during SSR", () => {
		setCsrfToken("tok");
		expect(getCsrfToken()).toBeNull();
	});
});
