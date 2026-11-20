import type { DevelopmentStatus, IndexEditionFields, IndexKind } from './api/generated';

/** How the ministry writes each status. */
export const statusLabels: Record<DevelopmentStatus, string> = {
  independent: 'Mandiri',
  advanced: 'Maju',
  developing: 'Berkembang',
  underdeveloped: 'Tertinggal',
  very_underdeveloped: 'Sangat Tertinggal',
};

/** Indeks Desa's six dimensions, in the ministry's order. */
export const dimensions = [
  ['basicServices', 'Layanan Dasar'],
  ['social', 'Sosial'],
  ['economic', 'Ekonomi'],
  ['environmental', 'Lingkungan'],
  ['accessibility', 'Aksesibilitas'],
  ['governance', 'Tata Kelola Pemerintahan'],
] as const;

/** Each index's top score and decimals; Indeks Desa's dimensions share its scale. */
export const scales: Record<IndexKind, { max: number; digits: number }> = {
  indeks_desa: { max: 100, digits: 2 },
  idm: { max: 1, digits: 4 },
};

/** A score as Indonesians write it, with its kind's decimals: 72,48 or 0,7142. */
export const formatScore = (score: number, kind: IndexKind) => {
  const { digits } = scales[kind];
  return score.toLocaleString('id-ID', {
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  });
};

/** "2025", or "2023 · IDM" for the older index. */
export const editionLabel = ({ year, kind }: Pick<IndexEditionFields, 'year' | 'kind'>) =>
  kind === 'idm' ? `${year} · IDM` : String(year);
