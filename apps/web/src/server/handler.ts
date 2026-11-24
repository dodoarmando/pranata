import { timingSafeEqual } from "node:crypto";
import { cspHeader, nonce, withNonce } from "./csp";
import type { HtmlCache } from "./html-cache";

export type HandlerDeps = {
	cache: HtmlCache;
	// Shared secret the API sends in `x-pranata-purge-token`. Empty disables purging.
	purgeToken: string;
	render: (request: Request) => Promise<Response>;
};

const PURGE_PATH = "/_internal/cache/purge";
const NEVER_CACHED = ["/admin", "/masuk", "/akses", "/_internal"];
const SESSION_COOKIE = /(?:^|;\s*)pranata_session=/;

function tokenMatches(given: string | null, expected: string): boolean {
	if (!given || !expected) {
		return false;
	}
	const a = Buffer.from(given);
	const b = Buffer.from(expected);
	return a.length === b.length && timingSafeEqual(a, b);
}

function underPrefix(path: string, prefix: string): boolean {
	return path === prefix || path.startsWith(`${prefix}/`);
}

function isCacheable(request: Request, path: string): boolean {
	if (request.method !== "GET") {
		return false;
	}
	if (SESSION_COOKIE.test(request.headers.get("cookie") ?? "")) {
		return false;
	}
	return !NEVER_CACHED.some((prefix) => underPrefix(path, prefix));
}

function htmlResponse(
	html: string,
	init: ResponseInit,
	source: Headers | null,
	cacheState: "HIT" | "MISS" | "BYPASS",
): Response {
	const n = nonce();
	const headers = new Headers(source ?? undefined);
	// The body length changes with the nonce, and the stream is re-encoded.
	headers.delete("content-length");
	headers.delete("content-encoding");
	headers.set("content-type", "text/html; charset=utf-8");
	headers.set("content-security-policy", cspHeader(n));
	headers.set("x-cache", cacheState);
	return new Response(withNonce(html, n), { ...init, headers });
}

export function createHandler(deps: HandlerDeps) {
	return async (request: Request): Promise<Response> => {
		const url = new URL(request.url);
		const path = url.pathname;

		if (underPrefix(path, "/_internal")) {
			if (
				path === PURGE_PATH &&
				request.method === "POST" &&
				tokenMatches(
					request.headers.get("x-pranata-purge-token"),
					deps.purgeToken,
				)
			) {
				deps.cache.purgeAll();
				return new Response(null, { status: 204 });
			}
			return new Response("Not found", { status: 404 });
		}

		const cacheable = isCacheable(request, path);
		const key = `${path}${url.search}`;

		if (cacheable) {
			const hit = deps.cache.get(key);
			if (hit !== undefined) {
				return htmlResponse(hit, { status: 200 }, null, "HIT");
			}
		}

		const response = await deps.render(request);
		const type = response.headers.get("content-type") ?? "";
		const state = cacheable ? "MISS" : "BYPASS";
		if (!type.includes("text/html")) {
			const headers = new Headers(response.headers);
			headers.set("x-cache", state);
			return new Response(response.body, {
				status: response.status,
				statusText: response.statusText,
				headers,
			});
		}

		const html = await response.text();
		if (cacheable && response.status === 200) {
			deps.cache.set(key, html);
		}
		return htmlResponse(
			html,
			{ status: response.status, statusText: response.statusText },
			response.headers,
			state,
		);
	};
}
