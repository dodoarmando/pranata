import { createFileRoute, getRouteApi, Link, Outlet } from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import { ChevronDown } from 'lucide-react';
import type { MouseEvent } from 'react';
import { listPublishedPages, type PageLink, type PageMenu } from '../api/generated';
import { VillageLogo } from '../components/village-logo';
import { pageMenus } from '../page';
import { regencyShort, villageTitle } from '../village';

// Server-only; empty when the API cannot be reached, so the site still has its menus.
const fetchPageLinks = createServerFn({ method: 'GET' }).handler(async (): Promise<PageLink[]> => {
  try {
    const response = await listPublishedPages();
    return response.status === 200 ? response.data : [];
  } catch {
    return [];
  }
});

const root = getRouteApi('__root__');

export const Route = createFileRoute('/_public')({
  loader: () => fetchPageLinks(),
  component: PublicLayout,
});

// The structured page each parent menu opens with, before its Pages.
const structured = {
  profile: { to: '/profil', label: 'Profil desa' },
  government: { to: '/pemerintahan', label: 'Struktur pemerintahan' },
  services: null,
} as const satisfies Record<PageMenu, { to: string; label: string } | null>;

// A native disclosure, so the menu needs no script; following a link closes it.
const closeMenu = (event: MouseEvent<HTMLElement>) =>
  event.currentTarget.closest('details')?.removeAttribute('open');

// Only shipped modules get navigation; each slice adds its own entry.
function PublicLayout() {
  const village = root.useLoaderData();
  const pageLinks = Route.useLoaderData();
  const logo = (
    <VillageLogo
      village={village}
      className="size-9.5 md:size-11"
      placeholderClassName="rounded-full text-[0.8125rem] font-bold md:text-[0.9375rem]"
    />
  );
  const navItem =
    'rounded-md px-2 py-1 text-[0.9375rem] font-medium data-[status=active]:text-primary md:px-3';
  return (
    <div className="backdrop flex min-h-dvh flex-col">
      <a
        href="#konten"
        className="absolute -top-12 left-(--pad) z-100 rounded-md bg-primary px-4 py-2 text-primary-foreground focus:top-2"
      >
        Langsung ke konten
      </a>
      {village && (
        <div className="hidden bg-pri-dd text-[0.8125rem] text-on-dark md:block">
          <div className="site-container py-2">Situs Resmi Pemerintah {villageTitle(village)}</div>
        </div>
      )}
      <header className="sticky top-3 z-50 my-3">
        <div className="site-container glass flex min-h-16 flex-wrap items-center justify-between gap-x-4 gap-y-1 rounded-md px-4 py-2 md:min-h-19">
          <Link to="/" className="flex items-center gap-3">
            {logo}
            {village ? (
              <span>
                <strong className="block text-[1.0625rem] leading-tight md:text-[1.1875rem]">
                  {villageTitle(village)}
                </strong>
                <small className="block text-xs text-muted-foreground md:text-[0.8125rem]">
                  {regencyShort(village)}, {village.province}
                </small>
              </span>
            ) : (
              <strong className="text-[1.0625rem] md:text-[1.1875rem]">Pranata</strong>
            )}
          </Link>
          <nav aria-label="Menu utama" className="flex flex-wrap">
            <Link to="/" className={navItem}>
              Beranda
            </Link>
            {(Object.keys(pageMenus) as PageMenu[]).map((menu) => {
              const home = structured[menu];
              const pages = pageLinks.filter((page) => page.menu === menu);
              if (pages.length === 0) {
                return (
                  home && (
                    <Link key={menu} to={home.to} className={navItem}>
                      {pageMenus[menu].label}
                    </Link>
                  )
                );
              }
              return (
                <details key={menu} className="relative">
                  <summary
                    className={`${navItem} flex cursor-pointer list-none items-center gap-1 [&::-webkit-details-marker]:hidden`}
                  >
                    {pageMenus[menu].label}
                    <ChevronDown className="size-4" aria-hidden="true" />
                  </summary>
                  <ul className="absolute left-0 z-10 mt-2 grid w-max max-w-72 gap-1 rounded-md border bg-popover p-2 text-popover-foreground shadow-lg">
                    {home && (
                      <li>
                        <Link
                          to={home.to}
                          activeOptions={{ exact: true }}
                          className={`${navItem} block`}
                          onClick={closeMenu}
                        >
                          {home.label}
                        </Link>
                      </li>
                    )}
                    {pages.map((page) => (
                      <li key={page.slug}>
                        <Link
                          to="/$menu/$slug"
                          params={{ menu: pageMenus[page.menu].path, slug: page.slug }}
                          className={`${navItem} block`}
                          onClick={closeMenu}
                        >
                          {page.title}
                        </Link>
                      </li>
                    ))}
                  </ul>
                </details>
              );
            })}
            <Link to="/berita" className={navItem}>
              Berita
            </Link>
            <Link to="/pengumuman" className={navItem}>
              Pengumuman
            </Link>
            <Link to="/agenda" className={navItem}>
              Agenda
            </Link>
            <Link to="/kontak" className={navItem}>
              Kontak
            </Link>
          </nav>
        </div>
      </header>
      <div id="konten" className="flex-1">
        <Outlet />
      </div>
      <footer className="bg-pri-dd text-sm text-on-dark">
        <div className="site-container">
          {village && (
            <div className="pt-12 pb-10 md:pt-16">
              <div className="mb-3.5 flex items-center gap-3">
                {logo}
                <strong className="text-[1.0625rem] text-white">
                  Pemerintah {villageTitle(village)}
                </strong>
              </div>
              <p>
                Kec. {village.district}, {regencyShort(village)}, {village.province}
              </p>
            </div>
          )}
          <div className="flex flex-wrap justify-between gap-x-6 gap-y-1 border-t border-white/10 pt-6 pb-8 text-[0.8125rem] text-on-dark-2">
            <p>Dikelola dengan Pranata</p>
            {village?.pseNumber && <p>Nomor Tanda Daftar PSE: {village.pseNumber}</p>}
          </div>
        </div>
      </footer>
    </div>
  );
}
