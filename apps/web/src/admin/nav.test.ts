import { describe, expect, it } from "@rstest/core";
import { visibleNav } from "./nav";

describe("visibleNav", () => {
	it("shows only the groups that have an enabled item", () => {
		const groups = visibleNav();
		expect(groups.map((g) => g.label)).toEqual(["Desa"]);
		expect(groups[0]?.items).toEqual([
			{ label: "Profil desa", to: "/admin/desa/profil" },
		]);
	});
});
