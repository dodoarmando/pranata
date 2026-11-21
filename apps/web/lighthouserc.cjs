// Lighthouse CI for `just perf`: the public site's performance budget at Lighthouse's default
// mobile profile (Moto G Power, 4× CPU slowdown, simulated Slow 4G), median of 3 runs, against
// the production images filled by `pranata seed`, behind Caddy as in production (HTTP/2,
// zstd). Sizes are transfer bytes.
// Raising a limit needs its own `perf(web): …` commit stating why.
const { chromium } = require('@playwright/test');

const origin = 'https://localhost:13443';
const code = process.env.PRANATA_PERF_COMPLAINT_CODE;
if (!code) throw new Error('Set PRANATA_PERF_COMPLAINT_CODE to the code `pranata seed` printed');

const median = (maxNumericValue) => ['error', { maxNumericValue, aggregationMethod: 'median' }];
const kB = 1000;

// Every measured page: the metrics and the per-resource weights.
const everyPage = {
  'largest-contentful-paint': median(2500),
  'total-blocking-time': median(200),
  'cumulative-layout-shift': median(0.1),
  'resource-summary:script:size': median(150 * kB),
  'resource-summary:stylesheet:size': median(30 * kB),
  'resource-summary:font:size': median(40 * kB),
  'resource-summary:document:size': median(50 * kB),
  'resource-summary:third-party:count': median(0),
};
const total = (limit) => ({ ...everyPage, 'resource-summary:total:size': median(limit * kB) });

module.exports = {
  ci: {
    collect: {
      url: [
        `${origin}/`,
        `${origin}/berita`,
        // The seed's long article.
        `${origin}/berita/musyawarah-desa-menyepakati-rencana-kerja-tahun-depan`,
        `${origin}/pengaduan`,
        `${origin}/pengaduan/status/${code}`,
      ],
      numberOfRuns: 3,
      chromePath: chromium.executablePath(),
      settings: { chromeFlags: '--headless=new --no-sandbox --ignore-certificate-errors' },
    },
    assert: {
      assertMatrix: [
        { matchingUrlPattern: '/$', assertions: total(500) },
        // The spec sets no page total for the News list.
        { matchingUrlPattern: '/berita$', assertions: everyPage },
        { matchingUrlPattern: '/berita/[^/]+$', assertions: total(400) },
        { matchingUrlPattern: '/pengaduan', assertions: total(300) },
      ],
    },
    upload: { target: 'filesystem', outputDir: '.lighthouseci' },
  },
};
