import type { Permission, SessionInfo } from "@/api/generated/model";

export const PERMISSION_LABELS: Record<Permission, string> = {
	"village.manage": "Mengelola data desa",
	"content.write": "Menulis konten",
	"content.publish": "Menerbitkan konten",
	"budget.manage": "Mengelola APBDes",
	"audit.read": "Melihat log audit",
	"user.manage": "Mengelola pengguna",
	"role.manage": "Mengelola peran",
};

export const PERMISSION_GROUPS: { label: string; permissions: Permission[] }[] =
	[
		{ label: "Desa", permissions: ["village.manage"] },
		{
			label: "Situs web",
			permissions: ["content.write", "content.publish", "budget.manage"],
		},
		{
			label: "Sistem",
			permissions: ["audit.read", "user.manage", "role.manage"],
		},
	];

export function can(
	session: SessionInfo | null,
	permission: Permission,
): boolean {
	return session?.user.permissions.includes(permission) ?? false;
}
