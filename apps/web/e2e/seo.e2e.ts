import { expect, test } from '@playwright/test';

// `just e2e-run` gives the web image the public origin the tests browse.
const origin = 'http://localhost:13000';

test('a filtered list is canonical to its page number alone, on the public origin', async ({
  page,
}) => {
  await page.goto('/berita?kategori=0193a6a0-0000-7000-8000-000000000000&halaman=2');
  await expect(page.locator('link[rel="canonical"]')).toHaveAttribute(
    'href',
    `${origin}/berita?halaman=2`,
  );
  await expect(page.locator('link[rel="alternate"][type="application/rss+xml"]')).toHaveAttribute(
    'href',
    `${origin}/berita/rss.xml`,
  );
});

test('search results are kept out of the index', async ({ page }) => {
  await page.goto('/cari?q=jalan');
  await expect(page.locator('meta[name="robots"]')).toHaveAttribute('content', 'noindex');
});

test('the sitemap, robots.txt and the feed are served', async ({ request }) => {
  const sitemap = await request.get('/sitemap.xml');
  expect(sitemap.headers()['content-type']).toContain('application/xml');
  expect(await sitemap.text()).toContain(`<loc>${origin}/berita</loc>`);

  const robots = await (await request.get('/robots.txt')).text();
  expect(robots).toContain('Disallow: /admin');
  expect(robots).toContain(`Sitemap: ${origin}/sitemap.xml`);

  const feed = await request.get('/berita/rss.xml');
  expect(feed.headers()['content-type']).toContain('application/rss+xml');
  expect(await feed.text()).toContain('<rss version="2.0"');
});

test('an unknown address is a 404 inside the site, linking to search', async ({ page }) => {
  expect((await page.goto('/tidak-ada'))?.status()).toBe(404);
  await expect(page.getByRole('heading', { name: 'Halaman tidak ditemukan' })).toBeVisible();
  await expect(page.getByRole('navigation', { name: 'Menu utama' })).toBeVisible();
  await expect(page.getByRole('link', { name: 'Cari di situs ini' })).toHaveAttribute(
    'href',
    '/cari',
  );
});
