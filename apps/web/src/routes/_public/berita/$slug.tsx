import { createFileRoute, getRouteApi, Link, notFound } from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import type { MouseEvent } from 'react';
import { getPublishedNewsPost, type Village } from '../../../api/generated';
import { newsDate } from '../../../news';
import { villageTitle } from '../../../village';

// Server-only: the body becomes HTML here, so Tiptap never ships to the browser. Null for a
// slug no Published News Post has; any other failure throws, so it is an error, not a 404.
const fetchPost = createServerFn({ method: 'GET' })
  .validator((slug: string) => slug)
  .handler(async ({ data }) => {
    const response = await getPublishedNewsPost(data);
    if (response.status === 404) return null;
    if (response.status !== 200) throw new Error('The API could not load the News Post');
    const { renderRichText } = await import('../../../rich-text-html');
    const { body, ...post } = response.data;
    return { ...post, html: renderRichText(body) };
  });

const root = getRouteApi('__root__');

export const Route = createFileRoute('/_public/berita/$slug')({
  loader: async ({ params }) => {
    const post = await fetchPost({ data: params.slug });
    if (!post) throw notFound();
    return post;
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
  component: NewsPostPage,
  notFoundComponent: NewsPostNotFound,
});

/** Turns a YouTube link in the body into a youtube-nocookie player, loading nothing before. */
function playYoutube(event: MouseEvent<HTMLElement>) {
  const link = (event.target as Element).closest<HTMLAnchorElement>('a[data-youtube]');
  if (!link?.dataset.youtube) return;
  event.preventDefault();
  const player = document.createElement('iframe');
  player.src = `https://www.youtube-nocookie.com/embed/${encodeURIComponent(link.dataset.youtube)}?autoplay=1`;
  player.title = 'Video YouTube';
  player.allow = 'autoplay; encrypted-media; picture-in-picture; fullscreen';
  player.className = 'aspect-video w-full rounded-md';
  link.replaceWith(player);
}

function NewsPostPage() {
  const village = root.useLoaderData();
  const post = Route.useLoaderData();
  return (
    <main className="site-container grid max-w-3xl gap-6 pt-10 pb-sec md:pt-16">
      <Link
        to="/berita"
        search={{ kategori: post.category.id }}
        className="w-fit font-medium text-primary"
      >
        {post.category.name}
      </Link>
      <h1 className="text-[2rem] leading-tight font-semibold tracking-tight md:text-[2.5rem]">
        {post.title}
      </h1>
      <p className="text-muted-foreground">
        {post.author.name}
        {post.author.position && `, ${post.author.position}`} ·{' '}
        {newsDate(post.publishedAt, village?.timeZone)} · {post.readingMinutes} menit baca
      </p>
      <figure className="grid gap-2">
        <img
          src={`/media/${post.cover}/960`}
          srcSet={`/media/${post.cover}/480 480w, /media/${post.cover}/960 960w, /media/${post.cover}/1600 1600w`}
          sizes="(min-width: 48rem) 48rem, 100vw"
          alt={post.coverAlt}
          width={960}
          height={540}
          fetchPriority="high"
          className="aspect-video w-full rounded-md object-cover"
        />
        {post.coverCaption && (
          <figcaption className="text-sm text-muted-foreground">{post.coverCaption}</figcaption>
        )}
      </figure>
      <p className="text-lg font-medium">{post.summary}</p>
      {/* biome-ignore lint/a11y/noStaticElementInteractions: delegates clicks on the YouTube links inside, which the keyboard reaches as links */}
      {/* biome-ignore lint/a11y/useKeyWithClickEvents: see above */}
      <div
        className="rich-text"
        onClick={playYoutube}
        // biome-ignore lint/security/noDangerouslySetInnerHtml: rendered on the server from the API's allowlisted JSON, with every text escaped
        dangerouslySetInnerHTML={{ __html: post.html }}
      />
    </main>
  );
}

function NewsPostNotFound() {
  return (
    <main className="site-container grid gap-4 pt-10 pb-sec md:pt-16">
      <h1 className="text-[2rem] leading-tight font-semibold tracking-tight">
        Berita tidak ditemukan
      </h1>
      <p className="text-muted-foreground">
        Berita ini mungkin sudah tidak diterbitkan atau alamatnya salah.
      </p>
      <p className="flex flex-wrap gap-4">
        <Link to="/berita" className="font-medium text-primary">
          Lihat semua berita
        </Link>
        <Link to="/" className="font-medium text-primary">
          Kembali ke beranda
        </Link>
      </p>
    </main>
  );
}
