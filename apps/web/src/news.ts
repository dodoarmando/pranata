import type { TimeZone } from './api/generated';
import { ianaZones } from './village';

/** A public date as Indonesians write it, "20 November 2026", in the Village's time zone. */
export const newsDate = (iso: string, zone: TimeZone | undefined) =>
  new Intl.DateTimeFormat('id-ID', {
    dateStyle: 'long',
    timeZone: ianaZones[zone ?? 'WIB'],
  }).format(new Date(iso));
