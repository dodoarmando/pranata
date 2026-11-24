const FALLBACK = "/admin";
const PROBE_ORIGIN = "http://probe.invalid";

// Accepts only same-origin relative paths under /admin. Anything else (absolute
// URLs, protocol-relative `//host`, backslash tricks, other sections) falls back
// to /admin so the `ke` parameter cannot become an open redirect.
export function safeAdminRedirect(raw: unknown): string {
	if (typeof raw !== "string") return FALLBACK;
	if (!/^\/admin(?:[/?#]|$)/.test(raw) || raw.includes("\\")) return FALLBACK;
	try {
		const url = new URL(raw, PROBE_ORIGIN);
		const stillAdmin =
			url.pathname === "/admin" || url.pathname.startsWith("/admin/");
		return url.origin === PROBE_ORIGIN && stillAdmin ? raw : FALLBACK;
	} catch {
		return FALLBACK;
	}
}
