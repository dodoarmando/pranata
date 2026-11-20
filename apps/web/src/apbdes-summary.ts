import type { BudgetLines, BudgetVersion } from './api/generated';

export const versionLabels: Record<BudgetVersion, string> = {
  original: 'Murni',
  amendment: 'Perubahan',
};

/** The 5 standard spending bidang, in their official order. */
export const bidang = [
  { line: 'governance', label: 'Penyelenggaraan Pemerintahan Desa' },
  { line: 'development', label: 'Pelaksanaan Pembangunan Desa' },
  { line: 'community', label: 'Pembinaan Kemasyarakatan Desa' },
  { line: 'empowerment', label: 'Pemberdayaan Masyarakat Desa' },
  { line: 'emergencies', label: 'Penanggulangan Bencana, Keadaan Darurat dan Mendesak Desa' },
] as const satisfies readonly { line: keyof BudgetLines; label: string }[];

/** Jumlah belanja: every bidang together. */
export const spending = (lines: BudgetLines) =>
  bidang.reduce((total, { line }) => total + lines[line], 0);

/** Pembiayaan netto: receipts less expenditures. */
export const netFinancing = (lines: BudgetLines) =>
  lines.financingReceipts - lines.financingExpenditures;

/** Surplus (or, negative, defisit): revenue less spending. */
export const surplus = (lines: BudgetLines) => lines.revenue - spending(lines);

const rupiahFormat = new Intl.NumberFormat('id-ID');

/** Whole rupiah as Indonesians write it: "Rp 2.420.000.000", "-Rp 5.000". */
export const rupiah = (amount: number) =>
  `${amount < 0 ? '-' : ''}Rp ${rupiahFormat.format(Math.abs(amount))}`;

/** How much of `whole` `part` is, as a whole percentage; 0 of nothing. */
export const share = (part: number, whole: number) =>
  whole > 0 ? Math.round((part / whole) * 100) : 0;
