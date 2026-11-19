import { expect, test } from '@rstest/core';
import { describeDevice } from './device';

test.each([
  ['Mozilla/5.0 (X11; Linux x86_64; rv:150.0) Gecko/20100101 Firefox/150.0', 'Firefox di Linux'],
  [
    'Mozilla/5.0 (Linux; Android 15) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0 Mobile Safari/537.36',
    'Chrome di Android',
  ],
  [
    'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0 Safari/537.36 Edg/150.0',
    'Edge di Windows',
  ],
  [
    'Mozilla/5.0 (iPhone; CPU iPhone OS 19_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/19.0 Mobile/15E148 Safari/604.1',
    'Safari di iOS',
  ],
  [
    'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/19.0 Safari/605.1.15',
    'Safari di macOS',
  ],
  ['curl/8.0', 'Perangkat tidak dikenal'],
  [null, 'Perangkat tidak dikenal'],
])('%s is %s', (userAgent, expected) => {
  expect(describeDevice(userAgent)).toBe(expected);
});
