import { getRouteApi, Link } from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import { Search } from 'lucide-react';
import { type SearchKind, type SearchPage, search } from '../api/generated';
import { newsDate } from '../news';
import { queryFits, resultPath, searchKinds } from '../search';
import { documentTypeLabels } from '../transparency-document';
import { Badge } from './ui/badge';
import { Button } from './ui/button';

type Query = { q: string; type?: SearchKind; page?: number };

/** Server-only; empty when the API cannot be reached. The query is never logged. */
export const fetchSearch = createServerFn({ method: 'GET' })
  .validator((query: Query) => query)
  .handler(async ({ data }): Promise<SearchPage> => {
    const empty = { results: [], total: 0, pageSize: 10 };
    try {
      const response = await search(data);
      return response.status === 200 ? response.data : empty;
    } catch {
      return empty;
    }
  });

/** One list page's search, scoped to `type`: null for a query outside 2–100 characters. */
export const searchOnly = async (type: SearchKind, q: string, page: number | undefined) => ({
  found: queryFits(q) ? await fetchSearch({ data: { q, type, ...(page && { page }) } }) : null,
});

/** A search box that submits `q` to `action`, so it works before any script loads. */
export function SearchForm({
  action,
  defaultValue,
  placeholder,
}: {
  action: string;
  defaultValue?: string | undefined;
  placeholder: string;
}) {
  return (
    <search>
      <form action={action} method="get" className="flex max-w-2xl gap-2">
        <label className="flex flex-1 items-center gap-2 rounded-md border bg-card px-3 focus-within:border-primary">
          <Search aria-hidden="true" className="size-4 text-muted-foreground" />
          <span className="sr-only">Cari</span>
          <input
            type="search"
            name="q"
            required
            minLength={2}
            maxLength={100}
            defaultValue={defaultValue}
            placeholder={placeholder}
            className="h-11 w-full bg-transparent outline-none"
          />
        </label>
        <Button type="submit" size="lg" className="h-11 px-5">
          Cari
        </Button>
      </form>
    </search>
  );
}

const root = getRouteApi('__root__');

/**
 * A page of results, a hint to check the spelling with links to the list pages, or, without a
 * page, the length the query must have.
 */
export function SearchResults({ page, q }: { page: SearchPage | null; q: string }) {
  const village = root.useLoaderData();
  if (!page) {
    return <p className="text-muted-foreground">Kata pencarian harus 2–100 karakter.</p>;
  }
  if (!page.results.length) {
    return (
      <p className="text-muted-foreground">
        Tidak ada hasil untuk “{q}”. Periksa ejaan kata pencarian, atau telusuri{' '}
        <Link to="/berita" className="font-medium text-primary">
          berita
        </Link>
        ,{' '}
        <Link to="/pengumuman" className="font-medium text-primary">
          pengumuman
        </Link>
        , dan{' '}
        <Link to="/dokumen" className="font-medium text-primary">
          dokumen
        </Link>
        .
      </p>
    );
  }
  return (
    <div className="grid gap-4">
      <p className="text-sm text-muted-foreground" aria-live="polite">
        {page.total} hasil untuk “{q}”
      </p>
      <ul className="grid gap-3">
        {page.results.map((result, index) => (
          // A static list the server orders; a place is unique where titles may repeat.
          // biome-ignore lint/suspicious/noArrayIndexKey: see above
          <li key={index} className="grid gap-1.5 rounded-md border bg-card p-5">
            <p className="flex flex-wrap items-center gap-2 text-sm text-muted-foreground">
              <Badge variant="outline">{searchKinds[result.kind].label}</Badge>
              {newsDate(result.datedAt, village?.timeZone)}
            </p>
            <h2 className="text-lg leading-snug font-semibold">
              <a href={resultPath(result)} className="hover:text-primary">
                {result.title}
              </a>
            </h2>
            <p className="line-clamp-2 text-muted-foreground">
              {result.documentType
                ? `${documentTypeLabels[result.documentType]} · Tahun anggaran ${result.fiscalYear}`
                : result.summary}
            </p>
          </li>
        ))}
      </ul>
    </div>
  );
}
