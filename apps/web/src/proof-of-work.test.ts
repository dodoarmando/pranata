import { createHash } from 'node:crypto';
import { expect, test } from '@rstest/core';
import type { Challenge } from './api/generated';
import { solve } from './proof-of-work';

/** A challenge as the API signs it, with `number` behind it. */
const challenge = (number: number, maxNumber = 1000): Challenge => {
  const salt = 'a1b2c3?expires=1900000000';
  const hash = createHash('sha256').update(`${salt}${number}`).digest('hex');
  return { algorithm: 'SHA-256', challenge: hash, maxNumber, salt, signature: 'signed' };
};

test('finds the number behind the challenge and keeps its salt and signature', async () => {
  expect(await solve(challenge(737))).toEqual({
    salt: 'a1b2c3?expires=1900000000',
    number: 737,
    signature: 'signed',
  });
});

test('tries 0 and maxNumber too', async () => {
  expect((await solve(challenge(0)))?.number).toBe(0);
  expect((await solve(challenge(1000)))?.number).toBe(1000);
});

test('gives up past maxNumber', async () => {
  expect(await solve(challenge(1001))).toBeNull();
});
