import type { Validity } from './api/generated';

export const validityLabels: Record<Validity, string> = {
  upcoming: 'Akan berlaku',
  inEffect: 'Berlaku',
  ended: 'Berakhir',
};

export const validityVariants = {
  upcoming: 'secondary',
  inEffect: 'default',
  ended: 'outline',
} as const satisfies Record<Validity, string>;

// A date-only value names a day, so it is read and written in UTC and no time zone shifts it.
const day = new Intl.DateTimeFormat('id-ID', { dateStyle: 'long', timeZone: 'UTC' });

/** "21 November 2026", or "1 November 2026 – 30 November 2026" for several days. */
export const period = (start: string, end: string) => {
  const from = day.format(new Date(`${start}T00:00:00Z`));
  return start === end ? from : `${from} – ${day.format(new Date(`${end}T00:00:00Z`))}`;
};
