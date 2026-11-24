// The CSRF token lives in memory only. Session cookies are HttpOnly, so the
// token comes from the login or getMe response and is set by the admin shell.
let token: string | null = null;

export function getCsrfToken(): string | null {
	return token;
}

export function setCsrfToken(t: string | null): void {
	token = t;
}
