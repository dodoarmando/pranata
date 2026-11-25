// Indonesian names for the `action` and `object_type` strings the API records
// in the audit log. `audit-labels.test.ts` compares both tables with the API
// source, so a new action or object type cannot ship without a label.
export const ACTION_LABELS: Record<string, string> = {
	create: "Membuat",
	update: "Mengubah",
	publish: "Menerbitkan",
	unpublish: "Membatalkan terbit",
	submit: "Mengajukan",
	deactivate: "Menonaktifkan",
	reactivate: "Mengaktifkan kembali",
	delete: "Menghapus",
	sign_in: "Masuk",
	sign_out: "Keluar",
	reauthenticate: "Konfirmasi ulang",
	issue_access_link: "Membuat tautan akses",
};

export const OBJECT_LABELS: Record<string, string> = {
	user: "Pengguna",
	role: "Peran",
	village: "Desa",
	media_asset: "Media",
	passkey: "Passkey",
};

function labelFrom(table: Record<string, string>, key: string): string {
	return Object.hasOwn(table, key) ? (table[key] ?? key) : key;
}

export function actionLabel(action: string): string {
	return labelFrom(ACTION_LABELS, action);
}

export function objectLabel(objectType: string): string {
	return labelFrom(OBJECT_LABELS, objectType);
}
