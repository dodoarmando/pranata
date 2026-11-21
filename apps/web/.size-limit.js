// The web job's fast guard on client JS (brotli). Lighthouse CI measures pages in `just perf`.
// Raising a limit needs its own `perf(web): …` commit stating why.
import { globSync, statSync } from 'node:fs';

const js = 'dist/client/assets/js/**/*.js';
const largest = globSync(js).reduce((a, b) => (statSync(a).size >= statSync(b).size ? a : b));

export default [
  { name: 'Client JS, every chunk', path: js, limit: '450 kB' },
  { name: `Largest chunk (${largest.split('/').pop()})`, path: largest, limit: '150 kB' },
  {
    name: 'Proof-of-work worker',
    path: 'dist/client/assets/js/async/proof-of-work.*.js',
    limit: '20 kB',
  },
];
