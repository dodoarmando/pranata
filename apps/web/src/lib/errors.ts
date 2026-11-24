const FALLBACK = "Terjadi kesalahan. Coba lagi.";

const LINK_INVALID =
	"Tautan ini sudah tidak berlaku. Minta tautan baru kepada Administrator.";

// Indonesian copy for every problem+json `code` the API can return.
const MESSAGES: Record<string, string> = {
	invalid_credentials: "Nama pengguna atau kata sandi salah.",
	access_link_used: LINK_INVALID,
	access_link_expired: LINK_INVALID,
	access_link_not_found: LINK_INVALID,
	password_length: "Kata sandi 10–128 karakter.",
	username_invalid: "Nama pengguna tidak valid.",
	unsupported_media_type:
		"Format foto tidak didukung. Gunakan JPEG, PNG, atau WebP.",
	file_too_large: "Ukuran berkas melebihi batas.",
	image_too_large: "Resolusi foto terlalu besar.",
	image_truncated: "Berkas foto rusak atau tidak lengkap. Unggah ulang.",
	image_unreadable:
		"Berkas ini bukan foto yang bisa dibaca. Gunakan foto lain.",
	image_undecodable: "Foto tidak dapat diproses. Gunakan foto lain.",
	username_taken: "Nama pengguna sudah dipakai. Pilih yang lain.",
	required: "Isian ini wajib diisi.",
	duplicate: "Isian ini hanya boleh dikirim satu kali.",
	invalid: "Isian tidak valid. Periksa kembali.",
	database_unavailable: "Layanan sedang terganggu. Coba lagi sebentar lagi.",
	invalid_multipart: "Unggahan tidak dapat dibaca. Coba lagi.",
	media_not_found: "Foto tidak ditemukan.",
	stale_write: "Data sudah diubah orang lain. Muat ulang halaman.",
	hero_not_ready: "Foto masih diproses. Tunggu sebentar.",
	overview_invalid: "Ringkasan desa tidak valid. Periksa kembali isiannya.",
	overview_too_large: "Ringkasan desa terlalu besar.",
	validation_failed: "Isian belum benar. Periksa kembali.",
	village_not_initialized: "Data desa belum disiapkan.",
	not_found: "Data tidak ditemukan.",
	unauthenticated: "Sesi berakhir. Silakan masuk kembali.",
	csrf_mismatch: "Sesi tidak valid. Muat ulang halaman lalu coba lagi.",
	internal_error: FALLBACK,
};

// The codes that have copy, for the test that compares them with the API.
export const MESSAGE_CODES: readonly string[] = Object.keys(MESSAGES);

export function messageFor(code: string): string {
	return Object.hasOwn(MESSAGES, code)
		? (MESSAGES[code] ?? FALLBACK)
		: FALLBACK;
}
