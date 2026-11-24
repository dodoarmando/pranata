import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "@rstest/core";
import { MESSAGE_CODES, messageFor } from "./errors";

const API_SRC = fileURLToPath(new URL("../../../api/src", import.meta.url));

function sources(dir: string): string[] {
	return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
		const path = join(dir, entry.name);
		if (entry.isDirectory()) return sources(path);
		return entry.name.endsWith(".rs") ? [path] : [];
	});
}

// Every problem code and field error code the API source can emit. The
// patterns are the four ways apps/api/src spells one; a new way means this
// test must learn it, so it fails loudly when it finds fewer than expected.
function apiCodes(): Set<string> {
	const codes = new Set<string>();
	const variant =
		/\b(?:NotFound|Unauthorized|Forbidden|Conflict|Gone|Unsupported|TooLarge|Unprocessable|Unavailable)\("([a-z_]+)"\)/g;
	const field = /\bcode: "([a-z_]+)"/g;
	const helper = /\binvalid\("[a-z_]+", "([a-z_]+)"\)/g;
	const parts = /StatusCode::[A-Z_]+,\s*"[^"]+",\s*"([a-z_]+)",?\s*\)/g;
	const matchBlock = /let code = match[\s\S]*?\n\s*\};/g;
	for (const file of sources(API_SRC)) {
		// Unit tests inside a source file may use made-up codes.
		const text = readFileSync(file, "utf8").split("#[cfg(test)]")[0] ?? "";
		for (const pattern of [variant, field, helper, parts]) {
			for (const m of text.matchAll(pattern)) codes.add(m[1] as string);
		}
		for (const block of text.matchAll(matchBlock)) {
			for (const m of block[0].matchAll(/=> "([a-z_]+)"/g)) {
				codes.add(m[1] as string);
			}
		}
	}
	return codes;
}

describe("messageFor", () => {
	it("maps known codes", () => {
		expect(messageFor("invalid_credentials")).toBe(
			"Nama pengguna atau kata sandi salah.",
		);
		expect(messageFor("stale_write")).toBe(
			"Data sudah diubah orang lain. Muat ulang halaman.",
		);
	});

	it("gives the link copy to every access link code", () => {
		const copy =
			"Tautan ini sudah tidak berlaku. Minta tautan baru kepada Administrator.";
		for (const code of [
			"access_link_used",
			"access_link_expired",
			"access_link_not_found",
		]) {
			expect(messageFor(code)).toBe(copy);
		}
	});

	it("falls back for unknown", () => {
		expect(messageFor("nope")).toBe("Terjadi kesalahan. Coba lagi.");
		expect(messageFor("constructor")).toBe("Terjadi kesalahan. Coba lagi.");
	});

	it("has Indonesian copy for every code the API can emit", () => {
		const codes = apiCodes();
		// A sanity floor: the scan found the codes the API is known to emit.
		for (const known of [
			"stale_write",
			"username_taken",
			"image_unreadable",
			"database_unavailable",
			"overview_too_large",
			"invalid_multipart",
			"validation_failed",
			"internal_error",
			"password_length",
			"required",
		]) {
			expect(codes.has(known), `scan missed ${known}`).toBe(true);
		}
		const missing = [...codes].filter((code) => !MESSAGE_CODES.includes(code));
		expect(missing).toEqual([]);
	});
});
