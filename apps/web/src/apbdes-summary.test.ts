import { expect, test } from '@rstest/core';
import { netFinancing, rupiah, share, spending, surplus } from './apbdes-summary';

const lines = {
  revenue: 2_420_000_000,
  governance: 792_000_000,
  development: 1_020_000_000,
  community: 160_000_000,
  empowerment: 300_000_000,
  emergencies: 105_000_000,
  financingReceipts: 45_000_000,
  financingExpenditures: 5_000_000,
};

test('totals spending over the 5 bidang and nets financing and the surplus', () => {
  expect(spending(lines)).toBe(2_377_000_000);
  expect(netFinancing(lines)).toBe(40_000_000);
  expect(surplus(lines)).toBe(43_000_000);
  expect(surplus({ ...lines, revenue: 0 })).toBe(-2_377_000_000);
});

test.each([
  [2_420_000_000, 'Rp 2.420.000.000'],
  [0, 'Rp 0'],
  [-5000, '-Rp 5.000'],
])('writes %d as %s', (amount, expected) => {
  expect(rupiah(amount)).toBe(expected);
});

test('rounds a share to a whole percentage and gives 0 of nothing', () => {
  expect(share(792, 2377)).toBe(33);
  expect(share(5, 0)).toBe(0);
});
