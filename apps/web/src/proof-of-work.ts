import type { Challenge, Proof } from './api/generated';

/**
 * Finds the number whose SHA-256 of the salt and its digits is the challenge, trying each up
 * to `maxNumber`; null when none is, which a valid challenge never does.
 */
export async function solve({ challenge, maxNumber, salt, signature }: Challenge) {
  const encoder = new TextEncoder();
  for (let number = 0; number <= maxNumber; number++) {
    const digest = await crypto.subtle.digest('SHA-256', encoder.encode(`${salt}${number}`));
    if (hex(digest) === challenge) return { salt, number, signature } satisfies Proof;
  }
  return null;
}

const hex = (digest: ArrayBuffer) =>
  Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, '0')).join('');
