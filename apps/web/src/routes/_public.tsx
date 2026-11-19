import { createFileRoute, getRouteApi, Link, Outlet } from '@tanstack/react-router';
import { regencyShort, villageInitial, villageTitle } from '../village';

const root = getRouteApi('__root__');

export const Route = createFileRoute('/_public')({
  component: PublicLayout,
});

// Only shipped modules get navigation; each slice adds its own entry.
function PublicLayout() {
  const village = root.useLoaderData();
  const logo = (
    <span
      className="grid size-9.5 flex-none place-items-center rounded-full bg-primary text-[0.8125rem] font-bold text-primary-foreground md:size-11 md:text-[0.9375rem]"
      aria-hidden="true"
    >
      {villageInitial(village)}
    </span>
  );
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
        <div className="site-container glass flex min-h-16 items-center justify-between gap-4 rounded-md px-4 md:min-h-19">
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
          <nav aria-label="Menu utama">
            <Link
              to="/"
              className="rounded-md px-3 py-1 text-[0.9375rem] font-medium data-[status=active]:text-primary"
            >
              Beranda
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
          <p className="border-t border-white/10 pt-6 pb-8 text-[0.8125rem] text-on-dark-2">
            Dikelola dengan Pranata
          </p>
        </div>
      </footer>
    </div>
  );
}
