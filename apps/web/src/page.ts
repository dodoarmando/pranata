import type { PageMenu } from './api/generated';

/** Each parent menu's label and its URL's first part, in the navigation's order. */
export const pageMenus = {
  profile: { label: 'Profil', path: 'profil' },
  government: { label: 'Pemerintahan', path: 'pemerintahan' },
  services: { label: 'Layanan', path: 'layanan' },
} as const satisfies Record<PageMenu, { label: string; path: string }>;

/** A Page's public address. */
export const pagePath = (menu: PageMenu, slug: string) => `/${pageMenus[menu].path}/${slug}`;
