import { describe, expect, it } from "@rstest/core";
import type { Permission } from "@/api/generated/model";
import { type NavGroup, visibleNav } from "./nav";

const ALL: Permission[] = [
	"village.manage",
	"content.write",
	"content.publish",
	"budget.manage",
	"audit.read",
	"user.manage",
	"role.manage",
];

describe("visibleNav", () => {
	it("shows only the groups that have an enabled item", () => {
		const groups = visibleNav(ALL);
		expect(groups.map((g) => g.label)).toEqual(["Desa", "Sistem"]);
		expect(groups[0]?.items).toEqual([
			{ label: "Profil desa", to: "/admin/desa/profil" },
		]);
		expect(groups[1]?.items).toEqual([
			{ label: "Pengguna", to: "/admin/sistem/pengguna" },
			{ label: "Peran", to: "/admin/sistem/peran" },
			{ label: "Log audit", to: "/admin/sistem/log-audit" },
		]);
	});

	it("hides items whose permission the user lacks", () => {
		expect(visibleNav(["content.write"])).toEqual([]);
	});

	it("shows every enabled item, including the Sistem group, with all permissions", () => {
		const enabled: NavGroup[] = [
			{
				label: "Sistem",
				icon: visibleNav(ALL)[0]?.icon as NavGroup["icon"],
				items: [
					{
						label: "Pengguna",
						to: "/admin/desa/profil",
						permission: "user.manage",
					},
					{
						label: "Peran",
						to: "/admin/desa/profil",
						permission: "role.manage",
					},
					{ label: "Umum", to: "/admin/desa/profil", permission: null },
				],
			},
		];
		expect(visibleNav(ALL, enabled)[0]?.items.map((i) => i.label)).toEqual([
			"Pengguna",
			"Peran",
			"Umum",
		]);
		expect(visibleNav([], enabled)[0]?.items.map((i) => i.label)).toEqual([
			"Umum",
		]);
	});
});
