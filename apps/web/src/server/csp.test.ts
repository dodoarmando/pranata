import { describe, expect, it } from "@rstest/core";
import { NONCE_PLACEHOLDER } from "@/lib/nonce";
import { cspHeader, nonce, withNonce } from "./csp";

describe("csp", () => {
	it("contains nonce and forbids framing", () => {
		const header = cspHeader("abc123==");
		expect(header).toContain("script-src 'self' 'nonce-abc123=='");
		expect(header).toContain("frame-ancestors 'none'");
		expect(header).toContain("frame-src https://www.youtube-nocookie.com");
		expect(header).toContain("base-uri 'none'");
	});

	it("generates 16 random bytes as base64", () => {
		const a = nonce();
		expect(Buffer.from(a, "base64")).toHaveLength(16);
		expect(nonce()).not.toBe(a);
	});

	it("substitutes every placeholder", () => {
		const html = `<script nonce="${NONCE_PLACEHOLDER}"></script><script nonce="${NONCE_PLACEHOLDER}"></script>`;
		const out = withNonce(html, "n1");
		expect(out).not.toContain(NONCE_PLACEHOLDER);
		expect(out.match(/nonce="n1"/g)).toHaveLength(2);
	});

	it("substitutes only nonce attributes, not page text", () => {
		const html = `<meta property="csp-nonce" content="${NONCE_PLACEHOLDER}"/><script nonce="${NONCE_PLACEHOLDER}"></script><p>${NONCE_PLACEHOLDER}</p><p>nonce=&quot;${NONCE_PLACEHOLDER}&quot;</p>`;
		const out = withNonce(html, "n1");
		expect(out).toContain('<script nonce="n1">');
		expect(out).toContain('content="n1"');
		expect(out).toContain(`<p>${NONCE_PLACEHOLDER}</p>`);
		expect(out).toContain(`&quot;${NONCE_PLACEHOLDER}&quot;`);
	});
});
