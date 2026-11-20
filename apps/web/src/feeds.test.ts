import { expect, test } from '@rstest/core';
import type { NewsCard, Sitemap } from './api/generated';
import { robotsTxt, rssXml, sitemapXml } from './feeds';

const origin = 'https://desa.example';

test('the sitemap lists the public pages and every Published item with its last change', () => {
  const sitemap: Sitemap = {
    newsPosts: [{ slug: 'jalan', updatedAt: '2026-11-01T03:00:00Z' }],
    announcements: [{ slug: 'kerja-bakti', updatedAt: '2026-11-02T03:00:00Z' }],
    pages: [{ menu: 'services', slug: 'ktp', updatedAt: '2026-11-03T03:00:00Z' }],
    albums: [{ slug: 'hut-ri', updatedAt: '2026-11-04T03:00:00Z' }],
    apbdesSummaries: [{ fiscalYear: 2026, updatedAt: '2026-11-05T03:00:00Z' }],
  };
  const xml = sitemapXml(origin, sitemap);
  expect(xml).toContain('<loc>https://desa.example/</loc>');
  expect(xml).toContain('<loc>https://desa.example/berita</loc>');
  for (const [path, lastmod] of [
    ['/berita/jalan', '2026-11-01T03:00:00Z'],
    ['/pengumuman/kerja-bakti', '2026-11-02T03:00:00Z'],
    ['/layanan/ktp', '2026-11-03T03:00:00Z'],
    ['/galeri/hut-ri', '2026-11-04T03:00:00Z'],
    ['/apbdes?tahun=2026', '2026-11-05T03:00:00Z'],
  ]) {
    expect(xml).toContain(`<url><loc>${origin}${path}</loc><lastmod>${lastmod}</lastmod></url>`);
  }
  expect(xml).not.toContain('/cari');
});

test('robots.txt keeps crawlers out of the admin area and sign-in and points to the sitemap', () => {
  const robots = robotsTxt(origin);
  expect(robots).toContain('Allow: /');
  for (const path of ['/admin', '/masuk', '/akses']) expect(robots).toContain(`Disallow: ${path}`);
  expect(robots).toContain('Sitemap: https://desa.example/sitemap.xml');
});

test('the feed holds the News Posts, escaped, newest first as given', () => {
  const post = (slug: string, title: string): NewsCard => ({
    slug,
    title,
    summary: 'Ringkasan <b>tebal</b> & lain',
    category: 'Kegiatan',
    cover: 'cover-id',
    coverAlt: 'Foto',
    publishedAt: '2026-11-21T03:00:00Z',
    readingMinutes: 2,
  });
  const xml = rssXml(origin, 'Desa Sukamaju', 'Situs resmi.', [
    post('baru', 'Jalan & jembatan'),
    post('lama', 'Lama'),
  ]);
  expect(xml).toContain('<rss version="2.0"');
  expect(xml).toContain('<title>Berita — Desa Sukamaju</title>');
  expect(xml).toContain('<title>Jalan &amp; jembatan</title>');
  expect(xml).toContain('<description>Ringkasan &lt;b&gt;tebal&lt;/b&gt; &amp; lain</description>');
  expect(xml).toContain('<link>https://desa.example/berita/baru</link>');
  expect(xml).toContain('<pubDate>Sat, 21 Nov 2026 03:00:00 GMT</pubDate>');
  expect(xml.indexOf('/berita/baru')).toBeLessThan(xml.indexOf('/berita/lama'));
});
