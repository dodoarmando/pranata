import { createFileRoute, Link } from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import { period } from '../../../announcement';
import { type GalleryPage, listPublishedAlbums } from '../../../api/generated';
import { pageHead } from '../../../seo';

type Search = { halaman?: number };

// Server-only; empty when the API cannot be reached.
const fetchGallery = createServerFn({ method: 'GET' })
  .validator((search: Search) => search)
  .handler(async ({ data }): Promise<GalleryPage> => {
    const empty = { albums: [], total: 0, pageSize: 12 };
    try {
      const page = await listPublishedAlbums(data.halaman ? { page: data.halaman } : {});
      return page.status === 200 ? page.data : empty;
    } catch {
      return empty;
    }
  });

export const Route = createFileRoute('/_public/galeri/')({
  validateSearch: (search: Record<string, unknown>): Search => {
    const page = Number(search.halaman);
    return Number.isInteger(page) && page > 1 ? { halaman: page } : {};
  },
  loaderDeps: ({ search }) => search,
  loader: ({ deps }) => fetchGallery({ data: deps }),
  head: (context) => pageHead(context, { title: 'Galeri' }),
  component: Gallery,
});

function Gallery() {
  const page = Route.useLoaderData();
  const current = Route.useSearch().halaman ?? 1;
  const pages = Math.max(1, Math.ceil(page.total / page.pageSize));
  return (
    <main className="site-container grid gap-8 pt-10 pb-sec md:pt-16">
      <h1 className="text-[2rem] leading-tight font-semibold tracking-tight md:text-[2.5rem]">
        Galeri
      </h1>
      {page.albums.length ? (
        <ul className="grid gap-6 sm:grid-cols-2 lg:grid-cols-3">
          {page.albums.map((album, index) => (
            <li key={album.slug} className="overflow-hidden rounded-md border bg-card">
              <Link to="/galeri/$slug" params={{ slug: album.slug }} className="group grid">
                <img
                  src={`/media/${album.cover}/480`}
                  srcSet={`/media/${album.cover}/480 480w, /media/${album.cover}/960 960w`}
                  sizes="(min-width: 1024px) 24rem, (min-width: 640px) 50vw, 100vw"
                  alt={album.coverAlt}
                  width={480}
                  height={320}
                  loading={index === 0 ? 'eager' : 'lazy'}
                  className="aspect-3/2 w-full object-cover"
                />
                <div className="grid gap-2 p-5">
                  <p className="text-sm text-muted-foreground">
                    {period(album.occasionStart, album.occasionEnd ?? album.occasionStart)} ·{' '}
                    {album.photoCount} foto
                  </p>
                  <h2 className="text-lg leading-snug font-semibold group-hover:text-primary">
                    {album.title}
                  </h2>
                </div>
              </Link>
            </li>
          ))}
        </ul>
      ) : (
        <p className="text-muted-foreground">Belum ada album.</p>
      )}
      {pages > 1 && (
        <nav aria-label="Halaman galeri" className="flex items-center justify-between gap-4">
          {current > 1 ? (
            <Link
              to="/galeri"
              search={current > 2 ? { halaman: current - 1 } : {}}
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
              to="/galeri"
              search={{ halaman: current + 1 }}
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
