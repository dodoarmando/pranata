import {
	FileTextIcon,
	GlobeIcon,
	LandmarkIcon,
	type LucideIcon,
	ShieldIcon,
	UsersIcon,
} from "lucide-react";
import type { Permission } from "@/api/generated/model";

// Routes that exist today. An item with `to: null` is planned (R1+) and is not
// rendered, so the menu never links to a dead page.
export type AdminPath = "/admin/desa/profil";

export interface NavItem {
	label: string;
	to: AdminPath | null;
	permission: Permission | null;
}

export interface NavGroup {
	label: string;
	icon: LucideIcon;
	items: NavItem[];
}

// Structure follows the admin prototype. Kependudukan and Surat belong to R2/R3.
export const NAV: NavGroup[] = [
	{
		label: "Desa",
		icon: LandmarkIcon,
		items: [
			{
				label: "Profil desa",
				to: "/admin/desa/profil",
				permission: "village.manage",
			},
			{ label: "Wilayah", to: null, permission: null },
			{ label: "Perangkat desa", to: null, permission: null },
			{ label: "Lembaga desa", to: null, permission: null },
			{ label: "Indeks Desa", to: null, permission: null },
		],
	},
	{
		label: "Kependudukan",
		icon: UsersIcon,
		items: [
			{ label: "Penduduk", to: null, permission: null },
			{ label: "Keluarga", to: null, permission: null },
			{ label: "Peristiwa", to: null, permission: null },
			{ label: "Rekap bulanan", to: null, permission: null },
			{ label: "Buku induk", to: null, permission: null },
			{ label: "Buku mutasi", to: null, permission: null },
		],
	},
	{
		label: "Surat",
		icon: FileTextIcon,
		items: [
			{ label: "Surat layanan", to: null, permission: null },
			{ label: "Surat tercatat", to: null, permission: null },
			{ label: "Buku agenda surat keluar", to: null, permission: null },
			{ label: "Jenis surat", to: null, permission: null },
			{ label: "Penomoran", to: null, permission: null },
		],
	},
	{
		label: "Situs web",
		icon: GlobeIcon,
		items: [
			{ label: "Hero beranda", to: null, permission: null },
			{ label: "Sambutan Kepala Desa", to: null, permission: null },
			{ label: "Berita", to: null, permission: null },
			{ label: "Pengumuman", to: null, permission: null },
			{ label: "Agenda", to: null, permission: null },
			{ label: "Halaman", to: null, permission: null },
			{ label: "Galeri", to: null, permission: null },
			{ label: "Kontak penting", to: null, permission: null },
			{ label: "Mitra", to: null, permission: null },
			{ label: "Dokumen transparansi", to: null, permission: null },
			{ label: "Ringkasan APBDes", to: null, permission: null },
		],
	},
	{
		label: "Sistem",
		icon: ShieldIcon,
		items: [
			{ label: "Pengguna", to: null, permission: "user.manage" },
			{ label: "Peran", to: null, permission: "role.manage" },
			{ label: "Log audit", to: null, permission: "audit.read" },
		],
	},
];

export interface VisibleGroup {
	label: string;
	icon: LucideIcon;
	items: { label: string; to: AdminPath }[];
}

// Items the User lacks the Permission for are dropped, and so are groups left
// with no enabled item.
export function visibleNav(
	permissions: readonly Permission[],
	groups: NavGroup[] = NAV,
): VisibleGroup[] {
	return groups.flatMap((group) => {
		const items = group.items.flatMap((item) =>
			item.to && (!item.permission || permissions.includes(item.permission))
				? [{ label: item.label, to: item.to }]
				: [],
		);
		return items.length > 0
			? [{ label: group.label, icon: group.icon, items }]
			: [];
	});
}
