import { describe, expect, it } from '@rstest/core';
import { schedule } from './event';

describe('schedule', () => {
  it('writes the wall-clock times as given, with the zone once', () => {
    expect(schedule('2026-11-21T08:00:00', null, 'WIB')).toBe('Sabtu, 21 November 2026, 08.00 WIB');
    expect(schedule('2026-11-21T08:00:00', '2026-11-21T12:30:00', 'WITA')).toBe(
      'Sabtu, 21 November 2026, 08.00–12.30 WITA',
    );
    expect(schedule('2026-11-21T08:00:00', '2026-11-23T17:00:00', 'WIT')).toBe(
      'Sabtu, 21 November 2026, 08.00 – Senin, 23 November 2026, 17.00 WIT',
    );
  });
});
