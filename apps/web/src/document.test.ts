import { expect, test } from '@rstest/core';
import { fileSize } from './document';

test.each([
  [200, '1 KB'],
  [980 * 1024, '980 KB'],
  [1024 * 1024, '1 MB'],
  [1.25 * 1024 * 1024, '1,3 MB'],
  [25 * 1024 * 1024, '25 MB'],
])('writes %d bytes as %s', (bytes, expected) => {
  expect(fileSize(bytes)).toBe(expected);
});
