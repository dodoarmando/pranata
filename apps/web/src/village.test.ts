import { expect, test } from '@rstest/core';
import { villageToday } from './village';

test("today is the Village's date, not UTC's", () => {
  const lateUtc = new Date('2026-11-20T18:30:00Z');
  expect(villageToday('WIB', lateUtc)).toBe('2026-11-21');
  expect(villageToday('WIT', new Date('2026-11-20T14:59:00Z'))).toBe('2026-11-20');
});
