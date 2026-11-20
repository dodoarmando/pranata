import type { TimeZone, Village } from './api/generated';

// Desa adat names already start with "Desa Adat".
export const villageTitle = (village: Village) =>
  village.name.startsWith('Desa Adat ') ? village.name : `Desa ${village.name}`;

/** The letter on the placeholder logo until the Village uploads its own. */
export const villageInitial = (village: Village | null) => village?.name.charAt(0) ?? 'P';

/** The Village Code as the ministry writes it, `11.01.01.2001`. */
export const villageCodeDotted = (code: string) =>
  `${code.slice(0, 2)}.${code.slice(2, 4)}.${code.slice(4, 6)}.${code.slice(6)}`;

/** A local word such as a Hamlet label, `dusun`, as a heading or label starts it. */
export const capitalized = (text: string) => text.charAt(0).toUpperCase() + text.slice(1);

export const regencyShort = (village: Village) => village.regency.replace(/^Kabupaten /, 'Kab. ');

/** The IANA zone of each Indonesian time zone, for formatting times as the Village reads them. */
export const ianaZones: Record<TimeZone, string> = {
  WIB: 'Asia/Jakarta',
  WITA: 'Asia/Makassar',
  WIT: 'Asia/Jayapura',
};

/** Today's `YYYY-MM-DD` in the Village's time zone, WIB before its code is set. */
export const villageToday = (zone: TimeZone = 'WIB', now = new Date()) =>
  new Intl.DateTimeFormat('en-CA', { timeZone: ianaZones[zone] }).format(now);
