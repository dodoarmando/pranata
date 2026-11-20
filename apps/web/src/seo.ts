import type { Village } from './api/generated';
import { villageTitle } from './village';

/** What the public layout's loader gives every page's head. */
export type Site = { origin: string; hero: string | null };

/** The part of a route's head context the tags are built from. */
type HeadContext = {
  matches: readonly { routeId: string; loaderData?: unknown }[];
  match: { pathname: string; search: unknown };
};

/** What one page says about itself; anything left out falls back to the Village. */
type PageSeo = {
  /** Without it, the Village's name alone, as on the home page. */
  title?: string;
  description?: string;
  /** A file id whose `share` variant is the image. */
  image?: string | null | undefined;
  type?: 'website' | 'article';
  jsonLd?: object;
};

/** The sentence a page without a summary is described by, from what the Village Code names. */
export const villageSentence = (village: Village) =>
  `Situs resmi Pemerintah ${villageTitle(village)}, Kecamatan ${village.district}, ` +
  `${village.regency}, Provinsi ${village.province}.`;

/** At most `max` characters, cut at a word with an ellipsis. */
export function truncate(text: string, max = 160) {
  if (text.length <= max) return text;
  const cut = text.slice(0, max);
  return `${cut.slice(0, cut.lastIndexOf(' ')).trimEnd()}…`;
}

/** JSON for a `<script>`, which `<` in the data cannot close. */
export const jsonLd = (data: object) => JSON.stringify(data).replace(/</g, '\\u003c');

// Only these name a different page; every other search parameter filters one.
const canonicalParams = ['halaman', 'tahun'];

/** The share variant of an image, absolute for link previews. */
export const shareImage = (origin: string, file: string) => `${origin}/media/${file}/share`;

/** The Village, read from the root match, and the public layout's Site. */
export function siteOf(matches: HeadContext['matches']) {
  // The root match comes first; its loader data is typed only through getRouteApi.
  const village = (matches[0]?.loaderData as Village | null | undefined) ?? null;
  const site = matches.find((match) => match.routeId === '/_public')?.loaderData as
    | Site
    | undefined;
  return { village, origin: site?.origin ?? '', hero: site?.hero ?? null };
}

/** The title, description, canonical URL, link preview and JSON-LD of one public page. */
export function pageHead({ matches, match }: HeadContext, page: PageSeo) {
  const { village, origin, hero } = siteOf(matches);
  const name = village && villageTitle(village);
  const title = page.title && name ? `${page.title} — ${name}` : (page.title ?? name ?? 'Pranata');
  const description = page.description ?? (village ? villageSentence(village) : undefined);
  const image = page.image ?? hero ?? village?.logo;
  const search = new URLSearchParams();
  for (const key of canonicalParams) {
    const value = (match.search as Record<string, unknown>)[key];
    if (value !== undefined) search.set(key, String(value));
  }
  // An index route's pathname ends in a slash the site's links never have.
  const path = match.pathname.replace(/(.)\/$/, '$1');
  const url = `${origin}${path}${search.size ? `?${search}` : ''}`;
  return {
    meta: [
      { title },
      ...(description
        ? [
            { name: 'description', content: description },
            { property: 'og:description', content: description },
          ]
        : []),
      { property: 'og:title', content: title },
      { property: 'og:type', content: page.type ?? 'website' },
      { property: 'og:url', content: url },
      { property: 'og:locale', content: 'id_ID' },
      ...(name ? [{ property: 'og:site_name', content: name }] : []),
      ...(image
        ? [
            { property: 'og:image', content: shareImage(origin, image) },
            { name: 'twitter:card', content: 'summary_large_image' },
          ]
        : []),
    ],
    links: [{ rel: 'canonical', href: url }],
    scripts: page.jsonLd ? [{ type: 'application/ld+json', children: jsonLd(page.jsonLd) }] : [],
  };
}

/** The Village's government as schema.org describes it, for the home page. */
export const governmentOrganization = (village: Village, origin: string) => ({
  '@context': 'https://schema.org',
  '@type': 'GovernmentOrganization',
  name: `Pemerintah ${villageTitle(village)}`,
  url: `${origin}/`,
  ...(village.logo && { logo: shareImage(origin, village.logo) }),
  ...(village.phone && { telephone: village.phone }),
  ...(village.email && { email: village.email }),
  address: {
    '@type': 'PostalAddress',
    ...(village.address && { streetAddress: village.address }),
    addressLocality: village.district,
    addressRegion: village.province,
    ...(village.postalCode && { postalCode: village.postalCode }),
    addressCountry: 'ID',
  },
});
