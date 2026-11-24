import { timingSafeEqual } from "node:crypto";
import { cspHeader, nonce, withNonce } from "./csp";
import type { HtmlCache } from "./html-cache";

export type HandlerDeps<Ctx> = {
	cache: HtmlCache;
	// Shared secret the API sends in `x-pranata-purge-token`. Empty disables purging.
	purgeToken: string;
	render: (request: Request, ctx: Ctx) => Promise<Response>;
};

const PURGE_PATH = "/_internal/cache/purge";
const PRIVATE_SEGMENTS = new Set(["admin", "masuk", "akses", "_internal"]);
const SESSION_COOKIE = /(?:^|;\s*)pranata_session=/;
const UNCACHEABLE_CONTROL = /\b(private|no-store|no-cache)\b/i;

type Rendered = {
	status: number;
	statusText: string;
	headers: Headers;
	html: string | null;
	body: ArrayBuffer | null;
};

// First path segment after percent-decoding, lowercasing and collapsing
// slashes; null when the encoding is malformed.
function firstSegment(pathname: string): string | null {
	let decoded: string;
	try {
		decoded = decodeURIComponent(pathname);
	} catch {
		return null;
	}
	const parts = decoded
		.toLowerCase()
		.split("/")
		.filter((part) => part !== "" && part !== ".");
	return parts[0] ?? "";
}

function tokenMatches(given: string | null, expected: string): boolean {
	if (!given || !expected) {
		return false;
	}
	const a = Buffer.from(given);
	const b = Buffer.from(expected);
	return a.length === b.length && timingSafeEqual(a, b);
}

// Safe to hand to another visitor: no cookie, not marked private.
function shareable(result: Rendered): boolean {
	return (
		!result.headers.has("set-cookie") &&
		!UNCACHEABLE_CONTROL.test(result.headers.get("cache-control") ?? "")
	);
}

function storable(result: Rendered): boolean {
	return result.status === 200 && result.html !== null && shareable(result);
}

async function buffer(response: Response): Promise<Rendered> {
	const isHtml = (response.headers.get("content-type") ?? "").includes(
		"text/html",
	);
	return {
		status: response.status,
		statusText: response.statusText,
		headers: response.headers,
		html: isHtml ? await response.text() : null,
		body: isHtml ? null : await response.arrayBuffer(),
	};
}

function toResponse(
	result: Rendered,
	cacheState: "HIT" | "MISS" | "BYPASS",
): Response {
	const headers = new Headers(result.headers);
	headers.set("x-cache", cacheState);
	const init = { status: result.status, statusText: result.statusText };
	if (result.html === null) {
		return new Response(result.body, { ...init, headers });
	}
	const n = nonce();
	// The body length changes with the nonce, and the stream is re-encoded.
	headers.delete("content-length");
	headers.delete("content-encoding");
	headers.set("content-type", "text/html; charset=utf-8");
	headers.set("content-security-policy", cspHeader(n));
	return new Response(withNonce(result.html, n), { ...init, headers });
}

export function createHandler<Ctx = undefined>(deps: HandlerDeps<Ctx>) {
	// Bumped by every purge: a render that began earlier must not repopulate.
	let generation = 0;
	const inflight = new Map<string, Promise<Rendered>>();

	const renderBuffered = async (request: Request, ctx: Ctx) =>
		buffer(await deps.render(request, ctx));

	return async (request: Request, ctx?: Ctx): Promise<Response> => {
		const context = ctx as Ctx;
		const url = new URL(request.url);
		const segment = firstSegment(url.pathname);

		if (segment === "_internal") {
			if (
				url.pathname === PURGE_PATH &&
				request.method === "POST" &&
				tokenMatches(
					request.headers.get("x-pranata-purge-token"),
					deps.purgeToken,
				)
			) {
				generation += 1;
				inflight.clear();
				deps.cache.purgeAll();
				return new Response(null, { status: 204 });
			}
			return new Response("Not found", { status: 404 });
		}

		const cacheable =
			request.method === "GET" &&
			segment !== null &&
			!PRIVATE_SEGMENTS.has(segment) &&
			!SESSION_COOKIE.test(request.headers.get("cookie") ?? "");

		if (!cacheable) {
			return toResponse(await renderBuffered(request, context), "BYPASS");
		}

		const key = `${url.pathname}${url.search}`;
		const hit = deps.cache.get(key);
		if (hit !== undefined) {
			return toResponse(
				{
					status: 200,
					statusText: "",
					headers: new Headers(),
					html: hit,
					body: null,
				},
				"HIT",
			);
		}

		// Single flight: concurrent misses for one key share a single render.
		const joined = inflight.get(key);
		if (joined) {
			const shared = await joined;
			if (shareable(shared)) {
				return toResponse(shared, "MISS");
			}
			// An uncacheable render (cookie, private) is never shared.
			return toResponse(await renderBuffered(request, context), "MISS");
		}

		const startedAt = generation;
		const flight = renderBuffered(request, context);
		inflight.set(key, flight);
		try {
			const result = await flight;
			if (storable(result) && generation === startedAt && result.html) {
				deps.cache.set(key, result.html);
			}
			return toResponse(result, "MISS");
		} finally {
			if (inflight.get(key) === flight) {
				inflight.delete(key);
			}
		}
	};
}
