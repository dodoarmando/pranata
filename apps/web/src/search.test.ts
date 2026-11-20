import { expect, test } from '@rstest/core';
import type { SearchResult } from './api/generated';
import { queryFits, resultPath, searchQuery } from './search';

test.each([
  [undefined, undefined],
  [42, undefined],
  ['  bangun  ', 'bangun'],
  ['a', 'a'],
  ['x'.repeat(150), 'x'.repeat(150)],
])('reads q=%s as %s', (raw, expected) => {
  expect(searchQuery(raw)).toBe(expected);
});

const result = (fields: Partial<SearchResult>): SearchResult => ({
  kind: 'newsPost',
  title: 'Judul',
  datedAt: '2026-11-20T00:00:00Z',
  ...fields,
});

test.each([
  [result({ slug: 'jalan' }), '/berita/jalan'],
  [result({ kind: 'announcement', slug: 'rapat' }), '/pengumuman/rapat'],
  [result({ kind: 'announcement' }), '/pengumuman'],
  [result({ kind: 'page', menu: 'government', slug: 'bpd' }), '/pemerintahan/bpd'],
  [result({ kind: 'transparencyDocument', file: 'f1' }), '/media/f1/pdf'],
])('links %o to %s', (item, path) => {
  expect(resultPath(item)).toBe(path);
});

test.each([
  ['a', false],
  ['ab', true],
  ['é'.repeat(100), true],
  ['😀'.repeat(100), true],
  ['a'.repeat(101), false],
])('takes %s: %s', (q, fits) => {
  expect(queryFits(q)).toBe(fits);
});
