import { describe, expect, it } from "@rstest/core";
import type { CurrentUser, RoleDetail } from "@/api/generated/model";
import { assignableRoles } from "./user-form-dialog";

const ROLES: RoleDetail[] = [
	{
		id: "admin",
		name: "Administrator",
		is_administrator: true,
		permissions: ["user.manage", "content.write"],
		user_count: 1,
	},
	{
		id: "writer",
		name: "Kontributor Web",
		is_administrator: false,
		permissions: ["content.write"],
		user_count: 0,
	},
	{
		id: "manager",
		name: "Pengelola Pengguna",
		is_administrator: false,
		permissions: ["user.manage"],
		user_count: 0,
	},
];

function user(role: RoleDetail): CurrentUser {
	return {
		id: "me",
		username: "me",
		display_name: "Me",
		role: {
			id: role.id,
			name: role.name,
			is_administrator: role.is_administrator,
		},
		permissions: role.permissions,
	};
}

describe("assignableRoles", () => {
	it("offers every Role to an Administrator", () => {
		expect(assignableRoles(ROLES, user(ROLES[0] as RoleDetail))).toHaveLength(
			3,
		);
	});

	it("leaves out the Administrator Role and Roles with Permissions the user lacks", () => {
		const names = assignableRoles(ROLES, user(ROLES[2] as RoleDetail)).map(
			(r) => r.name,
		);
		expect(names).toEqual(["Pengelola Pengguna"]);
	});
});
