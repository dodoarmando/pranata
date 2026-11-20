import { describe, expect, it } from '@rstest/core';
import { period } from './announcement';

describe('period', () => {
  it('names the days as written, whatever the time zone', () => {
    expect(period('2026-11-21', '2026-11-21')).toBe('21 November 2026');
    expect(period('2026-11-01', '2026-11-30')).toBe('1 November 2026 – 30 November 2026');
  });
});
