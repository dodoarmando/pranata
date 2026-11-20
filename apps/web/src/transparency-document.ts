import type {
  ListPublishedTransparencyDocumentsGroup,
  TransparencyDocumentType,
} from './api/generated';

/** Each catalog type as the public reads it. */
export const documentTypeLabels: Record<TransparencyDocumentType, string> = {
  apbdes: 'APBDes',
  apbdesAmendment: 'APBDes Perubahan',
  realisationReport: 'Laporan realisasi',
  rkpdes: 'RKPDes',
  rpjmdes: 'RPJMDes',
  villageRegulation: 'Perdes',
  other: 'Lainnya',
};

/** The public filter groups, in the chips' order, each with its `jenis` search value. */
export const documentGroups = [
  { slug: 'apbdes', group: 'budget', label: 'APBDes' },
  { slug: 'realisasi', group: 'realisation', label: 'Laporan realisasi' },
  { slug: 'perencanaan', group: 'planning', label: 'Perencanaan' },
  { slug: 'perdes', group: 'regulation', label: 'Peraturan desa' },
] as const satisfies readonly {
  slug: string;
  group: ListPublishedTransparencyDocumentsGroup;
  label: string;
}[];

const megabytes = new Intl.NumberFormat('id-ID', { maximumFractionDigits: 1 });

/** A file's size as Indonesians write it: 980 KB or 1,2 MB. */
export const fileSize = (bytes: number) =>
  bytes < 1 << 20
    ? `${Math.max(1, Math.round(bytes / 1024))} KB`
    : `${megabytes.format(bytes / (1 << 20))} MB`;
