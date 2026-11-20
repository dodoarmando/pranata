import type { Challenge } from './api/generated';
import { solve } from './proof-of-work';

// Off the main thread, so the form stays responsive on a slow phone while the proof is found.
addEventListener('message', async (event: MessageEvent<Challenge>) => {
  (self as unknown as Worker).postMessage(await solve(event.data));
});
