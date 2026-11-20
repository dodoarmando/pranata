import { createFileRoute, getRouteApi, Link } from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import {
  type Category,
  listCategories,
  listPublishedNewsPosts,
  type NewsPage,
  type Village,
} from '../../../api/generated';
import { newsDate } from '../../../news';
import { villageTitle } from '../../../village';

type Search = { kategori?: string; halaman?: number };

// Server-only; empty when the API cannot be reached.
const fetchNews = createServerFn({ method: 'GET' })
  .validator((search: Search) => search)
  .handler(async ({ data }): Promise<{ page: NewsPage; categories: Category[] }> => {
    const empty = { page: { posts: [], total: 0, pageSize: 9 }, categories: [] };
    try {
      const [page, categories] = await Promise.all([
        listPublishedNewsPosts({
          ...(data.kategori && { category: data.kategori }),
          ...(data.halaman && { page: data.halaman }),
        }),
        listCategories(),
      ]);
      return {
        page: page.status === 200 ? page.data : empty.page,
        categories: categories.status === 200 ? categories.data : [],
      };
    } catch {
      return empty;
    }
  });

const root = getRouteApi('__root__');

export const Route = createFileRoute('/_public/berita/')({
  validateSearch: (search: Record<string, unknown>): Search => {
    const page = Number(search.halaman);
    return {
      ...(typeof search.kategori === 'string' &&
        /^[0-9a-f-]{36}$/.test(search.kategori) && { kategori: search.kategori }),
      ...(Number.isInteger(page) && page > 1 && { halaman: page }),
    };
  },
  loaderDeps: ({ search }) => search,
  loader: ({ deps }) => fetchNews({ data: deps }),
  head: ({ matches }) => {
    // The root match comes first; its loader data is typed only through getRouteApi.
    const village = matches[0]?.loaderData as Village | null | undefined;
    return { meta: [{ title: village ? `Berita — ${villageTitle(village)}` : 'Berita' }] };
  },
  component: NewsList,
});

function NewsList() {
  const village = root.useLoaderData();
  const { page, categories } = Route.useLoaderData();
  const search = Route.useSearch();
  const current = search.halaman ?? 1;
  const pages = Math.max(1, Math.ceil(page.total / page.pageSize));
  const chip =
    'rounded-full border bg-card px-3.5 py-1.5 text-sm font-medium data-[status=active]:border-primary data-[status=active]:bg-primary data-[status=active]:text-primary-foreground';
  return (
    <main className="site-container grid gap-8 pt-10 pb-sec md:pt-16">
      <h1 className="text-[2rem] leading-tight font-semibold tracking-tight md:text-[2.5rem]">
        Berita
      </h1>
      {categories.length > 0 && (
        <nav aria-label="Kategori berita" className="flex flex-wrap gap-2">
          <Link to="/berita" activeOptions={{ includeSearch: true }} className={chip}>
            Semua
          </Link>
          {categories.map((category) => (
            <Link
              key={category.id}
              to="/berita"
              search={{ kategori: category.id }}
              activeOptions={{ includeSearch: true }}
              className={chip}
            >
              {category.name}
            </Link>
          ))}
        </nav>
      )}
      {page.posts.length ? (
        <ul className="grid gap-6 sm:grid-cols-2 lg:grid-cols-3">
          {page.posts.map((post, index) => (
            <li key={post.slug} className="overflow-hidden rounded-md border bg-card">
              <Link to="/berita/$slug" params={{ slug: post.slug }} className="group grid">
                <img
                  src={`/media/${post.cover}/480`}
                  srcSet={`/media/${post.cover}/480 480w, /media/${post.cover}/960 960w`}
                  sizes="(min-width: 1024px) 24rem, (min-width: 640px) 50vw, 100vw"
                  alt={post.coverAlt}
                  width={480}
                  height={270}
                  loading={index === 0 ? 'eager' : 'lazy'}
                  className="aspect-video w-full object-cover"
                />
                <div className="grid gap-2 p-5">
                  <p className="text-sm text-muted-foreground">
                    {post.category} · {newsDate(post.publishedAt, village?.timeZone)}
                  </p>
                  <h2 className="text-lg leading-snug font-semibold group-hover:text-primary">
                    {post.title}
                  </h2>
                  <p className="line-clamp-3 text-muted-foreground">{post.summary}</p>
                  <p className="text-sm text-muted-foreground">{post.readingMinutes} menit baca</p>
                </div>
              </Link>
            </li>
          ))}
        </ul>
      ) : (
        <p className="text-muted-foreground">Belum ada berita.</p>
      )}
      {pages > 1 && (
        <nav aria-label="Halaman berita" className="flex items-center justify-between gap-4">
          {current > 1 ? (
            <Link
              to="/berita"
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
              to="/berita"
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
