import { createFileRoute, Link, notFound } from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import { period } from '../../../announcement';
import { getPublishedAlbum, type Village } from '../../../api/generated';
import { villageTitle } from '../../../village';

// Server-only. Null for a slug no Published Album has; any other failure throws, so it is an
// error, not a 404.
const fetchAlbum = createServerFn({ method: 'GET' })
  .validator((slug: string) => slug)
  .handler(async ({ data }) => {
    const response = await getPublishedAlbum(data);
    if (response.status === 404) return null;
    if (response.status !== 200) throw new Error('The API could not load the Album');
    return response.data;
  });

export const Route = createFileRoute('/_public/galeri/$slug')({
  loader: async ({ params }) => {
    const album = await fetchAlbum({ data: params.slug });
    if (!album) throw notFound();
    return album;
  },
  head: ({ matches, loaderData }) => {
    // The root match comes first; its loader data is typed only through getRouteApi.
    const village = matches[0]?.loaderData as Village | null | undefined;
    if (!loaderData) return {};
    return {
      meta: [
        { title: village ? `${loaderData.title} — ${villageTitle(village)}` : loaderData.title },
        ...(loaderData.description
          ? [{ name: 'description', content: loaderData.description }]
          : []),
      ],
    };
  },
  component: AlbumPage,
  notFoundComponent: AlbumNotFound,
});

function AlbumPage() {
  const album = Route.useLoaderData();
  return (
    <main className="site-container grid gap-6 pt-10 pb-sec md:pt-16">
      <Link to="/galeri" className="w-fit font-medium text-primary">
        Galeri
      </Link>
      <h1 className="text-[2rem] leading-tight font-semibold tracking-tight md:text-[2.5rem]">
        {album.title}
      </h1>
      <p className="text-muted-foreground">
        {period(album.occasionStart, album.occasionEnd ?? album.occasionStart)} ·{' '}
        {album.photos.length} foto
      </p>
      {album.description && <p className="max-w-3xl text-lg">{album.description}</p>}
      <ul className="grid gap-6 sm:grid-cols-2 lg:grid-cols-3">
        {album.photos.map((photo, index) => (
          <li key={photo.file}>
            <figure className="grid gap-2">
              {/* The largest rendition, for a closer look. */}
              <a href={`/media/${photo.file}/1600`} target="_blank" rel="noopener">
                <img
                  src={`/media/${photo.file}/480`}
                  srcSet={`/media/${photo.file}/480 480w, /media/${photo.file}/960 960w, /media/${photo.file}/1600 1600w`}
                  sizes="(min-width: 1024px) 24rem, (min-width: 640px) 50vw, 100vw"
                  alt={photo.alt}
                  width={480}
                  height={320}
                  loading={index < 3 ? 'eager' : 'lazy'}
                  className="aspect-3/2 w-full rounded-md object-cover"
                />
              </a>
              {photo.caption && (
                <figcaption className="text-sm text-muted-foreground">{photo.caption}</figcaption>
              )}
            </figure>
          </li>
        ))}
      </ul>
    </main>
  );
}

function AlbumNotFound() {
  return (
    <main className="site-container grid gap-4 pt-10 pb-sec md:pt-16">
      <h1 className="text-[2rem] leading-tight font-semibold tracking-tight">
        Album tidak ditemukan
      </h1>
      <p className="text-muted-foreground">
        Album ini mungkin sudah tidak diterbitkan atau alamatnya salah.
      </p>
      <p className="flex flex-wrap gap-4">
        <Link to="/galeri" className="font-medium text-primary">
          Lihat semua album
        </Link>
        <Link to="/" className="font-medium text-primary">
          Kembali ke beranda
        </Link>
      </p>
    </main>
  );
}
