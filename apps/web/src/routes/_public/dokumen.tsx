import { createFileRoute, Link, useNavigate } from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import {
  listPublishedTransparencyDocuments,
  type TransparencyDocumentPage,
  type Village,
} from '../../api/generated';
import { TransparencyDocumentItem } from '../../components/transparency-document-item';
import { NativeSelect, NativeSelectOption } from '../../components/ui/native-select';
import { documentGroups } from '../../transparency-document';
import { villageTitle } from '../../village';

type Search = { jenis?: (typeof documentGroups)[number]['slug']; tahun?: number; halaman?: number };

// Server-only; empty when the API cannot be reached.
const fetchDocuments = createServerFn({ method: 'GET' })
  .validator((search: Search) => search)
  .handler(async ({ data }): Promise<TransparencyDocumentPage> => {
    const empty = { documents: [], total: 0, pageSize: 10, fiscalYears: [] };
    const group = documentGroups.find(({ slug }) => slug === data.jenis)?.group;
    try {
      const response = await listPublishedTransparencyDocuments({
        ...(group && { group }),
        ...(data.tahun && { fiscalYear: data.tahun }),
        ...(data.halaman && { page: data.halaman }),
      });
      return response.status === 200 ? response.data : empty;
    } catch {
      return empty;
    }
  });

export const Route = createFileRoute('/_public/dokumen')({
  validateSearch: (search: Record<string, unknown>): Search => {
    const jenis = documentGroups.find(({ slug }) => slug === search.jenis)?.slug;
    const year = Number(search.tahun);
    const page = Number(search.halaman);
    return {
      ...(jenis && { jenis }),
      ...(Number.isInteger(year) && year >= 2000 && year <= 2100 && { tahun: year }),
      ...(Number.isInteger(page) && page > 1 && { halaman: page }),
    };
  },
  loaderDeps: ({ search }) => search,
  loader: ({ deps }) => fetchDocuments({ data: deps }),
  head: ({ matches }) => {
    // The root match comes first; its loader data is typed only through getRouteApi.
    const village = matches[0]?.loaderData as Village | null | undefined;
    return { meta: [{ title: village ? `Dokumen — ${villageTitle(village)}` : 'Dokumen' }] };
  },
  component: DocumentList,
});

function DocumentList() {
  const page = Route.useLoaderData();
  const search = Route.useSearch();
  const navigate = useNavigate({ from: '/dokumen' });
  const current = search.halaman ?? 1;
  const pages = Math.max(1, Math.ceil(page.total / page.pageSize));
  const chip =
    'rounded-full border bg-card px-3.5 py-1.5 text-sm font-medium data-[status=active]:border-primary data-[status=active]:bg-primary data-[status=active]:text-primary-foreground';
  return (
    <main className="site-container grid gap-8 pt-10 pb-sec md:pt-16">
      <div className="grid max-w-2xl gap-3">
        <h1 className="text-[2rem] leading-tight font-semibold tracking-tight md:text-[2.5rem]">
          Dokumen transparansi
        </h1>
        <p className="text-muted-foreground md:text-lg">
          Dokumen anggaran, perencanaan, dan peraturan desa yang resmi berlaku, bebas diunduh oleh
          siapa saja.
        </p>
      </div>
      <div className="flex flex-wrap items-center justify-between gap-4">
        <nav aria-label="Jenis dokumen" className="flex flex-wrap gap-2">
          <Link
            to="/dokumen"
            search={({ jenis: _, halaman: __, ...rest }) => rest}
            activeOptions={{ includeSearch: true }}
            className={chip}
          >
            Semua
          </Link>
          {documentGroups.map(({ slug, label }) => (
            <Link
              key={slug}
              to="/dokumen"
              search={({ halaman: _, ...rest }) => ({ ...rest, jenis: slug })}
              activeOptions={{ includeSearch: true }}
              className={chip}
            >
              {label}
            </Link>
          ))}
        </nav>
        {page.fiscalYears.length > 0 && (
          <div className="flex items-center gap-2">
            <label htmlFor="fiscal-year" className="text-sm text-muted-foreground">
              Tahun anggaran
            </label>
            <NativeSelect
              id="fiscal-year"
              value={search.tahun ?? ''}
              onChange={(event) => {
                const tahun = Number(event.currentTarget.value) || undefined;
                navigate({
                  search: ({ tahun: _, halaman: __, ...rest }) => ({
                    ...rest,
                    ...(tahun && { tahun }),
                  }),
                });
              }}
            >
              <NativeSelectOption value="">Semua tahun</NativeSelectOption>
              {page.fiscalYears.map((year) => (
                <NativeSelectOption key={year} value={year}>
                  {year}
                </NativeSelectOption>
              ))}
            </NativeSelect>
          </div>
        )}
      </div>
      {page.documents.length ? (
        <ul className="grid gap-3">
          {page.documents.map((document, index) => (
            // A static list the server orders; a place is unique where titles may repeat.
            // biome-ignore lint/suspicious/noArrayIndexKey: see above
            <TransparencyDocumentItem key={index} document={document} />
          ))}
        </ul>
      ) : (
        <p className="text-muted-foreground">
          {search.jenis || search.tahun ? 'Belum ada dokumen yang cocok.' : 'Belum ada dokumen.'}
        </p>
      )}
      {pages > 1 && (
        <nav aria-label="Halaman dokumen" className="flex items-center justify-between gap-4">
          {current > 1 ? (
            <Link
              from="/dokumen"
              to="/dokumen"
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
              to="/dokumen"
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
