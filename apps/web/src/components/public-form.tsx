import { useRouter } from '@tanstack/react-router';
import { useEffect, useState } from 'react';
import type { Challenge, Proof } from '../api/generated';

/** A public form's pages and their status pages: not content to index, nor to keep. */
export const privatePage = {
  meta: [{ name: 'robots', content: 'noindex' }],
  headers: { 'cache-control': 'no-store' },
};

/**
 * The proof of work for `challenge`, solved in a Web Worker from when the form opens. The page
 * reloads its data before the API's 2-hour lifetime ends, so a form left open never sends an
 * expired proof, which the API would answer as if received and store nowhere.
 */
export function useProof(challenge: Challenge) {
  const router = useRouter();
  const [proof, setProof] = useState<Proof | null>(null);
  useEffect(() => {
    setProof(null);
    const worker = new Worker(new URL('../proof-of-work.worker.ts', import.meta.url), {
      name: 'proof-of-work',
    });
    worker.addEventListener('message', (event: MessageEvent<Proof | null>) => setProof(event.data));
    worker.postMessage(challenge);
    const timer = setTimeout(() => void router.invalidate(), 90 * 60_000);
    return () => {
      worker.terminate();
      clearTimeout(timer);
    };
  }, [challenge, router]);
  return proof;
}

/** A field people never see or reach and bots fill in, sent as `website`. */
export function Honeypot() {
  return (
    <div aria-hidden="true" className="absolute -left-[9999px] size-px overflow-hidden">
      <label>
        Situs web
        <input name="website" tabIndex={-1} autoComplete="off" />
      </label>
    </div>
  );
}
