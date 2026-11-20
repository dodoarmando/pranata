import type { TimeZone } from './api/generated';

// Event times are wall-clock times in the Village's zone, so they are read and written in UTC
// and no time zone shifts them.
const day = new Intl.DateTimeFormat('id-ID', {
  weekday: 'long',
  day: 'numeric',
  month: 'long',
  year: 'numeric',
  timeZone: 'UTC',
});
const clock = new Intl.DateTimeFormat('id-ID', { timeStyle: 'short', timeZone: 'UTC' });
const at = (dateTime: string) => new Date(`${dateTime}Z`);

/**
 * "Sabtu, 21 November 2026, 08.00 WIB", with "–12.30" for an end that day or the whole end
 * for one on a later day.
 */
export function schedule(start: string, end: string | null | undefined, zone: TimeZone) {
  const from = `${day.format(at(start))}, ${clock.format(at(start))}`;
  if (!end) return `${from} ${zone}`;
  return end.slice(0, 10) === start.slice(0, 10)
    ? `${from}–${clock.format(at(end))} ${zone}`
    : `${from} – ${day.format(at(end))}, ${clock.format(at(end))} ${zone}`;
}
