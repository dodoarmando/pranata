import { expect, test } from '@rstest/core';
import type { Village } from './api/generated';
import { jsonLd, pageHead, truncate } from './seo';

const village = {
  code: '1101012001',
  name: 'Sukamaju',
  district: 'Teupah Selatan',
  regency: 'Kabupaten Simeulue',
  province: 'Aceh',
  timeZone: 'WIB',
  logo: 'logo-id',
} as Village;

const context = (
  pathname: string,
  search: Record<string, unknown> = {},
  hero: string | null = null,
) => ({
  matches: [
    { routeId: '__root__', loaderData: village },
    { routeId: '/_public', loaderData: { origin: 'https://desa.example', hero, pageLinks: [] } },
  ],
  match: { pathname, search },
});

const meta = (head: ReturnType<typeof pageHead>, key: string) =>
  head.meta.find(
    (tag) => ('name' in tag && tag.name === key) || ('property' in tag && tag.property === key),
  )?.content;

test('the title names the Village and the description comes from the summary', () => {
  const head = pageHead(context('/berita/jalan'), { title: 'Jalan', description: 'Diaspal.' });
  expect(head.meta[0]).toEqual({ title: 'Jalan — Desa Sukamaju' });
  expect(meta(head, 'description')).toBe('Diaspal.');
  expect(meta(head, 'og:description')).toBe('Diaspal.');
});

test('without a summary the description is the sentence the Village Code gives', () => {
  const head = pageHead(context('/berita'), { title: 'Berita' });
  expect(meta(head, 'description')).toBe(
    'Situs resmi Pemerintah Desa Sukamaju, Kecamatan Teupah Selatan, Kabupaten Simeulue, Provinsi Aceh.',
  );
});

test('the canonical URL is absolute on the public origin and keeps only the page number', () => {
  const head = pageHead(context('/berita/', { kategori: 'abc', q: 'jalan', halaman: 2 }), {
    title: 'Berita',
  });
  expect(head.links).toContainEqual({
    rel: 'canonical',
    href: 'https://desa.example/berita?halaman=2',
  });
  expect(meta(head, 'og:url')).toBe('https://desa.example/berita?halaman=2');
  const filtered = pageHead(context('/dokumen', { tahun: 2025 }), { title: 'Dokumen' });
  expect(filtered.links).toContainEqual({ rel: 'canonical', href: 'https://desa.example/dokumen' });
  const year = pageHead(context('/apbdes', { tahun: 2025 }), { canonicalSearch: ['tahun'] });
  expect(year.links).toContainEqual({
    rel: 'canonical',
    href: 'https://desa.example/apbdes?tahun=2025',
  });
});

test('the share image is the cover, then the hero, then the logo', () => {
  const share = (id: string) => `https://desa.example/media/${id}/share`;
  expect(meta(pageHead(context('/', {}, 'hero-id'), { image: 'cover-id' }), 'og:image')).toBe(
    share('cover-id'),
  );
  expect(meta(pageHead(context('/', {}, 'hero-id'), {}), 'og:image')).toBe(share('hero-id'));
  const logo = pageHead(context('/'), {});
  expect(meta(logo, 'og:image')).toBe(share('logo-id'));
  expect(meta(logo, 'twitter:card')).toBe('summary_large_image');
});

test('the home page is titled by the Village alone', () => {
  expect(pageHead(context('/'), {}).meta[0]).toEqual({ title: 'Desa Sukamaju' });
});

test('long text is cut at a word to about 160 characters', () => {
  const text = 'kata '.repeat(50).trim();
  const cut = truncate(text);
  expect(cut.length).toBeLessThanOrEqual(160);
  expect(cut.endsWith('kata…')).toBe(true);
  expect(truncate('Pendek.')).toBe('Pendek.');
});

test('JSON-LD cannot close its script element', () => {
  expect(jsonLd({ name: '</script><script>alert(1)</script>' })).not.toContain('</script>');
});
