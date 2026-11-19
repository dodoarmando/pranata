import type { PasswordProblem } from './api/generated';

/** The password policy, shown wherever a User chooses a password. */
export const passwordHint =
  '12–128 karakter. Jangan memakai kata sandi umum, nama pengguna, atau nama desa.';

export const passwordProblems: Record<PasswordProblem, string> = {
  tooShort: 'Kata sandi minimal 12 karakter.',
  tooLong: 'Kata sandi paling banyak 128 karakter.',
  common: 'Kata sandi ini terlalu umum atau pernah bocor. Pilih yang lain.',
  containsUsername: 'Kata sandi tidak boleh memuat nama pengguna.',
  containsVillageName: 'Kata sandi tidak boleh memuat nama desa.',
};
