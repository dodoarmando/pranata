import type { TenureKind } from './api/generated';

/** How a Tenure's type is written in Indonesian. */
export const tenureKindLabels: Record<TenureKind, string> = {
  definitive: 'Definitif',
  appointee: 'Pj.',
  acting: 'Plt.',
};

const dateFormat = new Intl.DateTimeFormat('id-ID', { dateStyle: 'long', timeZone: 'UTC' });

/** A `YYYY-MM-DD` date as Indonesians write it, such as 1 Januari 2025. */
export const formatDate = (date: string) => dateFormat.format(new Date(`${date}T00:00:00Z`));

/** A serving Official's title: the Position, after Pj. or Plt. for a temporary holder. */
export const holderTitle = (kind: TenureKind, position: string) =>
  kind === 'definitive' ? position : `${tenureKindLabels[kind]} ${position}`;
