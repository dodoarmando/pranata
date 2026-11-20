import { expect, test } from '@rstest/core';
import { formatScore } from './index-edition';

test.each([
  [72.48, 'indeks_desa', '72,48'],
  [81, 'indeks_desa', '81,00'],
  [100, 'indeks_desa', '100,00'],
  [0.7142, 'idm', '0,7142'],
  [0.7, 'idm', '0,7000'],
] as const)('formats %d of %s as %s', (score, kind, expected) => {
  expect(formatScore(score, kind)).toBe(expected);
});
