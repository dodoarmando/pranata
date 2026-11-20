import { createFileRoute, Link } from '@tanstack/react-router';
import type { SearchKind, Village } from '../../api/generated';
import { fetchSearch, SearchForm, SearchResults } from '../../components/search-results';
import { queryFits, searchKinds, searchQuery } from '../../search';
import { villageTitle } from '../../village';

type Jenis = (typeof searchKinds)[SearchKind]['slug'];
type Search = { q?: string; jenis?: Jenis; halaman?: number };

const kinds = Object.entries(searchKinds) as [SearchKind, (typeof searchKinds)[SearchKind]][];
const kindOf = (jenis: Jenis) => kinds.find(([, { slug }]) => slug === jenis)?.[0];

export const Route = createFileRoute('/_public/cari')({
  validateSearch: (search: Record<string, unknown>): Search => {
    const q = searchQuery(search.q);
    const jenis = kinds.find(([, { slug }]) => slug === search.jenis)?.[1].slug;
    const page = Number(search.halaman);
    return {
      ...(q && { q }),
      ...(jenis && { jenis }),
      ...(Number.isInteger(page) && page > 1 && { halaman: page }),
    };
  },
  loaderDeps: ({ search }) => search,
  loader: ({ deps: { q, jenis, halaman } }) => {
    const type = jenis && kindOf(jenis);
    return q && queryFits(q)
      ? fetchSearch({ data: { q, ...(type && { type }), ...(halaman && { page: halaman }) } })
      : null;
  },
  head: ({ matches }) => {
    // The root match comes first; its loader data is typed only through getRouteApi.
    const village = matches[0]?.loaderData as Village | null | undefined;
    return {
      meta: [
        { title: village ? `Pencarian — ${villageTitle(village)}` : 'Pencarian' },
        // Result pages are not content of their own.
        { name: 'robots', content: 'noindex' },
      ],
    };
  },
  component: SiteSearch,
});

function SiteSearch() {
  const page = Route.useLoaderData();
  const search = Route.useSearch();
  const { q } = search;
  const current = search.halaman ?? 1;
  const pages = page ? Math.max(1, Math.ceil(page.total / page.pageSize)) : 1;
  const chip =
    'rounded-full border bg-card px-3.5 py-1.5 text-sm font-medium data-[status=active]:border-primary data-[status=active]:bg-primary data-[status=active]:text-primary-foreground';
  return (
    <main className="site-container grid gap-8 pt-10 pb-sec md:pt-16">
      <div className="grid gap-5">
        <h1 className="text-[2rem] leading-tight font-semibold tracking-tight md:text-[2.5rem]">
          Pencarian
        </h1>
        <SearchForm
          action="/cari"
          defaultValue={q}
          placeholder="Cari berita, pengumuman, halaman, atau dokumen…"
        />
      </div>
      {q && !page && <SearchResults page={null} q={q} />}
      {q && page && (
        <>
          <nav aria-label="Jenis hasil" className="flex flex-wrap gap-2">
            <Link
              to="/cari"
              search={{ q }}
              activeOptions={{ includeSearch: true }}
              className={chip}
            >
              Semua
            </Link>
            {kinds.map(([kind, { slug, label }]) => (
              <Link
                key={kind}
                to="/cari"
                search={{ q, jenis: slug }}
                activeOptions={{ includeSearch: true }}
                className={chip}
              >
                {label}
              </Link>
            ))}
          </nav>
          <SearchResults page={page} q={q} />
        </>
      )}
      {pages > 1 && (
        <nav aria-label="Halaman hasil" className="flex items-center justify-between gap-4">
          {current > 1 ? (
            <Link
              from="/cari"
              to="/cari"
              search={({ halaman: _, ...rest }) =>
                current > 2 ? { ...rest, halaman: current - 1 } : rest
              }
              className="font-medium text-primary"
            >
              ← Sebelumnya
            </Link>
          ) : (
            <span />
          )}
          <span className="text-muted-foreground">
            Halaman {current} dari {pages}
          </span>
          {current < pages ? (
            <Link
              to="/cari"
              search={{ ...search, halaman: current + 1 }}
              className="font-medium text-primary"
            >
              Berikutnya →
            </Link>
          ) : (
            <span />
          )}
        </nav>
      )}
    </main>
  );
}
