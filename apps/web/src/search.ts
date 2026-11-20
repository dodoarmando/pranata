import type { SearchKind, SearchResult } from './api/generated';
import { pagePath } from './page';

/** Each result kind's label and its `jenis` value, in the chips' order. */
export const searchKinds = {
  newsPost: { slug: 'berita', label: 'Berita' },
  announcement: { slug: 'pengumuman', label: 'Pengumuman' },
  page: { slug: 'halaman', label: 'Halaman' },
  transparencyDocument: { slug: 'dokumen', label: 'Dokumen' },
} as const satisfies Record<SearchKind, { slug: string; label: string }>;

/** A `q` search value, trimmed; blank or missing is no query. */
export const searchQuery = (raw: unknown) => (typeof raw === 'string' && raw.trim()) || undefined;

/** Whether the API takes the query: 2–100 characters, as it counts them. */
export const queryFits = (q: string) => [...q].length >= 2 && [...q].length <= 100;

/** Where a result opens: its page, the Announcement list when it has none, or the PDF. */
export const resultPath = (result: SearchResult) => {
  switch (result.kind) {
    case 'newsPost':
      return `/berita/${result.slug}`;
    case 'announcement':
      return result.slug ? `/pengumuman/${result.slug}` : '/pengumuman';
    case 'page':
      return result.menu && result.slug ? pagePath(result.menu, result.slug) : '/';
    case 'transparencyDocument':
      return `/media/${result.file}/pdf`;
  }
};
