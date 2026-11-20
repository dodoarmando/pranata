import type { NewsCard, Sitemap } from './api/generated';
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

/** `/berita/rss.xml`: RSS 2.0 of the given News Posts, in their order. */
export function rssXml(
  origin: string,
  villageName: string,
  description: string,
  posts: NewsCard[],
) {
  const items = posts.map((post) => {
    const link = `${origin}/berita/${post.slug}`;
    return `<item>
<title>${xml(post.title)}</title>
<link>${link}</link>
<guid isPermaLink="true">${link}</guid>
<description>${xml(post.summary)}</description>
<category>${xml(post.category)}</category>
<pubDate>${new Date(post.publishedAt).toUTCString()}</pubDate>
</item>`;
  });
  return `<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:atom="http://www.w3.org/2005/Atom">
<channel>
<title>Berita — ${xml(villageName)}</title>
<link>${origin}/berita</link>
<description>${xml(description)}</description>
<language>id</language>
<atom:link href="${origin}/berita/rss.xml" rel="self" type="application/rss+xml"/>
${items.join('\n')}
</channel>
</rss>
`;
}
