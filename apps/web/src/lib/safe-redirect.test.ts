import { describe, expect, it } from "@rstest/core";
import { safeAdminRedirect } from "./safe-redirect";

describe("safeAdminRedirect", () => {
	it("keeps same-origin admin paths", () => {
		expect(safeAdminRedirect("/admin")).toBe("/admin");
		expect(safeAdminRedirect("/admin/desa/profil")).toBe("/admin/desa/profil");
		expect(safeAdminRedirect("/admin/desa?x=1#a")).toBe("/admin/desa?x=1#a");
	});

	it.each([
		"https://evil.example/admin",
		"//evil.example/admin",
		"/\\evil.example",
		"/admin\\..\\evil",
		"/adminx",
		"/admin/../masuk",
		"/masuk",
		"javascript:alert(1)",
		"",
	])("falls back for %s", (value) => {
		expect(safeAdminRedirect(value)).toBe("/admin");
	});

	it("falls back for non-strings", () => {
		expect(safeAdminRedirect(undefined)).toBe("/admin");
		expect(safeAdminRedirect(["/admin"])).toBe("/admin");
	});
});
