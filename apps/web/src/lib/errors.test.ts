import { describe, expect, it } from "@rstest/core";
import { messageFor } from "./errors";

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
});
