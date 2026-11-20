// A date-only value names a day, so it is read and written in UTC and no time zone shifts it.
const day = new Intl.DateTimeFormat('id-ID', { dateStyle: 'long', timeZone: 'UTC' });

/** A date-only value as Indonesians write it, "21 November 2026". */
export const calendarDate = (date: string) => day.format(new Date(`${date}T00:00:00Z`));

/** "21 November 2026", or "1 November 2026 – 30 November 2026" for several days. */
export const period = (start: string, end: string) =>
  start === end ? calendarDate(start) : `${calendarDate(start)} – ${calendarDate(end)}`;
