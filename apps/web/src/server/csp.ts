import { NONCE_PLACEHOLDER } from "@/lib/nonce";

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

// Only the attributes Start fills with the placeholder are rewritten, so page
// content that happens to contain the token is never touched (a nonce in text
// would leak into the document, and an attacker could not predict it anyway).
const NONCE_ATTR = new RegExp(
	`(\\snonce="|<meta property="csp-nonce" content=")${NONCE_PLACEHOLDER}"`,
	"g",
);

export function withNonce(html: string, n: string): string {
	return html.replace(NONCE_ATTR, (_m, lead: string) => `${lead}${n}"`);
}
