import type { Village } from './api/generated';

// Desa adat names already start with "Desa Adat".
export const villageTitle = (village: Village) =>
  village.name.startsWith('Desa Adat ') ? village.name : `Desa ${village.name}`;

export const regencyShort = (village: Village) => village.regency.replace(/^Kabupaten /, 'Kab. ');
