import { describe, expect, it } from "@rstest/core";
import { expiryRedirect } from "./session-expiry";

describe("expiryRedirect", () => {
	it("returns to the current admin path after sign-in", () => {
		expect(expiryRedirect("unauthenticated", "/admin/desa/profil?x=1")).toBe(
			"/masuk?ke=%2Fadmin%2Fdesa%2Fprofil%3Fx%3D1",
		);
	});

	it("ignores other codes and non-admin pages", () => {
		expect(expiryRedirect("invalid_credentials", "/admin")).toBeNull();
		expect(expiryRedirect("unauthenticated", "/masuk")).toBeNull();
		expect(expiryRedirect("unauthenticated", "/adminx")).toBeNull();
	});
});
