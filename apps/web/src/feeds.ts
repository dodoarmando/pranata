import type { Sitemap } from './api/generated';
import { pagePath } from './page';

/** Text safe inside an XML element. */
const xml = (text: string) =>
  text.replace(/[&<>]/g, (char) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;' })[char] ?? char);

// The public pages that are not one item's; `/cari` is left out, being `noindex`.
const listPaths = [
  '/',
  '/profil',
  '/pemerintahan',
  '/berita',
  '/pengumuman',
  '/agenda',
  '/galeri',
  '/dokumen',
  '/apbdes',
  '/kontak',
];

/** `/sitemap.xml`: the public pages, then every Published item with a page and its last change. */
export function sitemapXml(origin: string, sitemap: Sitemap) {
  const items = [
    ...sitemap.newsPosts.map((post) => [`/berita/${post.slug}`, post.updatedAt]),
    ...sitemap.announcements.map((item) => [`/pengumuman/${item.slug}`, item.updatedAt]),
    ...sitemap.pages.map((page) => [pagePath(page.menu, page.slug), page.updatedAt]),
    ...sitemap.albums.map((album) => [`/galeri/${album.slug}`, album.updatedAt]),
    ...sitemap.apbdesSummaries.map((summary) => [
      `/apbdes?tahun=${summary.fiscalYear}`,
      summary.updatedAt,
    ]),
  ];
  const urls = [
    ...listPaths.map((path) => `<url><loc>${origin}${path}</loc></url>`),
    ...items.map(
      ([path, lastmod]) =>
        `<url><loc>${xml(`${origin}${path}`)}</loc><lastmod>${lastmod}</lastmod></url>`,
    ),
  ];
  return `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
${urls.join('\n')}
</urlset>
`;
}

/** `/robots.txt`: everything but the admin area and sign-in, and where the sitemap is. */
export const robotsTxt = (origin: string) => `User-agent: *
Allow: /
Disallow: /admin
Disallow: /masuk
Disallow: /akses

Sitemap: ${origin}/sitemap.xml
`;
