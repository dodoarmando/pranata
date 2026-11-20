import type { DocumentType, ListPublishedTransparencyDocumentsGroup } from './api/generated';

/** Each catalog type as the public reads it. */
export const documentTypeLabels: Record<DocumentType, string> = {
  apbdes: 'APBDes',
  apbdesAmendment: 'APBDes Perubahan',
  realisationReport: 'Laporan realisasi',
  rkpdes: 'RKPDes',
  rpjmdes: 'RPJMDes',
  villageRegulation: 'Perdes',
  other: 'Lainnya',
};

/** The public filter groups, in the chips' order, with their `jenis` search value. */
export const documentGroups = [
  ['apbdes', 'budget', 'APBDes'],
  ['realisasi', 'realisation', 'Laporan realisasi'],
  ['perencanaan', 'planning', 'Perencanaan'],
  ['perdes', 'regulation', 'Peraturan desa'],
] as const satisfies readonly (readonly [
  string,
  ListPublishedTransparencyDocumentsGroup,
  string,
])[];

const megabytes = new Intl.NumberFormat('id-ID', { maximumFractionDigits: 1 });

/** A file's size as Indonesians write it: 980 KB or 1,2 MB. */
export const fileSize = (bytes: number) =>
  bytes < 1 << 20
    ? `${Math.max(1, Math.round(bytes / 1024))} KB`
    : `${megabytes.format(bytes / (1 << 20))} MB`;
