import {
	FileTextIcon,
	GlobeIcon,
	LandmarkIcon,
	type LucideIcon,
	ShieldIcon,
	UsersIcon,
} from "lucide-react";

// Routes that exist today. An item with `to: null` is planned (R1+) and is not
// rendered, so the menu never links to a dead page.
export type AdminPath = "/admin/desa/profil";

export interface NavItem {
	label: string;
	to: AdminPath | null;
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
			{ label: "Profil desa", to: "/admin/desa/profil" },
			{ label: "Wilayah", to: null },
			{ label: "Perangkat desa", to: null },
			{ label: "Lembaga desa", to: null },
			{ label: "Indeks Desa", to: null },
		],
	},
	{
		label: "Kependudukan",
		icon: UsersIcon,
		items: [
			{ label: "Penduduk", to: null },
			{ label: "Keluarga", to: null },
			{ label: "Peristiwa", to: null },
			{ label: "Rekap bulanan", to: null },
			{ label: "Buku induk", to: null },
			{ label: "Buku mutasi", to: null },
		],
	},
	{
		label: "Surat",
		icon: FileTextIcon,
		items: [
			{ label: "Surat layanan", to: null },
			{ label: "Surat tercatat", to: null },
			{ label: "Buku agenda surat keluar", to: null },
			{ label: "Jenis surat", to: null },
			{ label: "Penomoran", to: null },
		],
	},
	{
		label: "Situs web",
		icon: GlobeIcon,
		items: [
			{ label: "Hero beranda", to: null },
			{ label: "Sambutan Kepala Desa", to: null },
			{ label: "Berita", to: null },
			{ label: "Pengumuman", to: null },
			{ label: "Agenda", to: null },
			{ label: "Halaman", to: null },
			{ label: "Galeri", to: null },
			{ label: "Kontak penting", to: null },
			{ label: "Mitra", to: null },
			{ label: "Dokumen transparansi", to: null },
			{ label: "Ringkasan APBDes", to: null },
		],
	},
	{
		label: "Sistem",
		icon: ShieldIcon,
		items: [
			{ label: "Pengguna", to: null },
			{ label: "Peran", to: null },
			{ label: "Log audit", to: null },
		],
	},
];

export interface VisibleGroup {
	label: string;
	icon: LucideIcon;
	items: { label: string; to: AdminPath }[];
}

// Groups with no enabled item are dropped entirely.
export function visibleNav(groups: NavGroup[] = NAV): VisibleGroup[] {
	return groups.flatMap((group) => {
		const items = group.items.flatMap((item) =>
			item.to ? [{ label: item.label, to: item.to }] : [],
		);
		return items.length > 0
			? [{ label: group.label, icon: group.icon, items }]
			: [];
	});
}
