// The SSR render writes this token wherever a nonce is needed; the server entry
// swaps in a fresh nonce for every response, cached or not.
export const NONCE_PLACEHOLDER = "__PRANATA_NONCE__";

export function nonce(): string {
	return Buffer.from(crypto.getRandomValues(new Uint8Array(16))).toString(
		"base64",
	);
}

export function cspHeader(n: string): string {
	return [
		"default-src 'self'",
		`script-src 'self' 'nonce-${n}'`,
		"style-src 'self' 'unsafe-inline'",
		"img-src 'self' data:",
		"connect-src 'self'",
		"frame-src https://www.youtube-nocookie.com",
		"frame-ancestors 'none'",
		"base-uri 'none'",
		"form-action 'self'",
	].join("; ");
}

export function withNonce(html: string, n: string): string {
	return html.replaceAll(NONCE_PLACEHOLDER, n);
}
