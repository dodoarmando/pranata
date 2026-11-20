import type { Institution } from './api/generated';

/** "Badan Permusyawaratan Desa (BPD)", or the bare name without an abbreviation. */
export const institutionTitle = ({
  name,
  abbreviation,
}: Pick<Institution, 'name' | 'abbreviation'>) =>
  abbreviation ? `${name} (${abbreviation})` : name;
