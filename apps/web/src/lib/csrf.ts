// Client-only. The CSRF token lives in module memory; on the server that
// module is shared across requests, so storing a token there would leak it
// between visitors. Session cookies are HttpOnly, so the token comes from the
// login or getMe response and is set by the admin shell in the browser.
let token: string | null = null;

export function getCsrfToken(): string | null {
	return token;
}

export function setCsrfToken(t: string | null): void {
	// No-op during SSR so a token can never be shared across requests.
	if (typeof window === "undefined") return;
	token = t;
}
