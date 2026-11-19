import type { TimeZone, Village } from './api/generated';

// Desa adat names already start with "Desa Adat".
export const villageTitle = (village: Village) =>
  village.name.startsWith('Desa Adat ') ? village.name : `Desa ${village.name}`;

export const regencyShort = (village: Village) => village.regency.replace(/^Kabupaten /, 'Kab. ');

/** The IANA zone of each Indonesian time zone, for formatting times as the Village reads them. */
export const ianaZones: Record<TimeZone, string> = {
  WIB: 'Asia/Jakarta',
  WITA: 'Asia/Makassar',
  WIT: 'Asia/Jayapura',
};
