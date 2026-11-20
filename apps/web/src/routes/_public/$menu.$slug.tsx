import { createFileRoute, getRouteApi, Link, notFound } from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import { getPublishedPage, type Village } from '../../api/generated';
import { RichTextBody } from '../../components/rich-text-body';
import { newsDate } from '../../news';
import { pageMenus } from '../../page';
import { villageTitle } from '../../village';

// Server-only: the body becomes HTML here, so Tiptap never ships to the browser. Null for a
// slug no Published Page has; any other failure throws, so it is an error, not a 404.
const fetchPage = createServerFn({ method: 'GET' })
  .validator((slug: string) => slug)
  .handler(async ({ data }) => {
    const response = await getPublishedPage(data);
    if (response.status === 404) return null;
    if (response.status !== 200) throw new Error('The API could not load the Page');
    const { renderRichText } = await import('../../rich-text-html');
    const { body, ...page } = response.data;
    return { ...page, html: renderRichText(body) };
  });

const root = getRouteApi('__root__');

export const Route = createFileRoute('/_public/$menu/$slug')({
  loader: async ({ params }) => {
    // Any other first part, such as a Page's slug under the wrong menu, is not a Page.
    const page = await fetchPage({ data: params.slug });
    if (!page || pageMenus[page.menu].path !== params.menu) throw notFound();
    return page;
  },
  head: ({ matches, loaderData }) => {
    // The root match comes first; its loader data is typed only through getRouteApi.
    const village = matches[0]?.loaderData as Village | null | undefined;
    if (!loaderData) return {};
    return {
      meta: [
        { title: village ? `${loaderData.title} — ${villageTitle(village)}` : loaderData.title },
        { name: 'description', content: loaderData.summary },
      ],
    };
  },
  component: PageView,
  notFoundComponent: PageNotFound,
});

function PageView() {
  const village = root.useLoaderData();
  const page = Route.useLoaderData();
  return (
    <main className="site-container grid max-w-3xl gap-6 pt-10 pb-sec md:pt-16">
      <p className="font-medium text-primary">{pageMenus[page.menu].label}</p>
      <h1 className="text-[2rem] leading-tight font-semibold tracking-tight md:text-[2.5rem]">
        {page.title}
      </h1>
      {page.cover && (
        <figure className="grid gap-2">
          <img
            src={`/media/${page.cover}/960`}
            srcSet={`/media/${page.cover}/480 480w, /media/${page.cover}/960 960w, /media/${page.cover}/1600 1600w`}
            sizes="(min-width: 48rem) 48rem, 100vw"
            alt={page.coverAlt ?? ''}
            width={960}
            height={540}
            fetchPriority="high"
            className="aspect-video w-full rounded-md object-cover"
          />
          {page.coverCaption && (
            <figcaption className="text-sm text-muted-foreground">{page.coverCaption}</figcaption>
          )}
        </figure>
      )}
      <p className="text-lg font-medium">{page.summary}</p>
      <RichTextBody html={page.html} />
      <p className="text-sm text-muted-foreground">
        Terakhir diperbarui {newsDate(page.updatedAt, village?.timeZone)}
      </p>
    </main>
  );
}

function PageNotFound() {
  return (
    <main className="site-container grid gap-4 pt-10 pb-sec md:pt-16">
      <h1 className="text-[2rem] leading-tight font-semibold tracking-tight">
        Halaman tidak ditemukan
      </h1>
      <p className="text-muted-foreground">
        Halaman ini mungkin sudah tidak diterbitkan atau alamatnya salah.
      </p>
      <p>
        <Link to="/" className="font-medium text-primary">
          Kembali ke beranda
        </Link>
      </p>
    </main>
  );
}
