import { createFileRoute, getRouteApi, Link, notFound } from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import { getPublishedNewsPost } from '../../../api/generated';
import { NotFound } from '../../../components/not-found';
import { RichTextBody } from '../../../components/rich-text-body';
import { newsDate } from '../../../news';
import { governmentOrganization, pageHead, shareImage, siteOf } from '../../../seo';

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
  head: (context) => {
    const post = context.loaderData;
    if (!post) return {};
    const { village, origin } = siteOf(context.matches);
    return pageHead(context, {
      title: post.title,
      description: post.summary,
      image: post.cover,
      type: 'article',
      jsonLd: {
        '@context': 'https://schema.org',
        '@type': 'NewsArticle',
        headline: post.title,
        description: post.summary,
        image: [shareImage(origin, post.cover)],
        datePublished: post.publishedAt,
        author: { '@type': 'Person', name: post.author.name },
        ...(village && { publisher: governmentOrganization(village, origin) }),
        mainEntityOfPage: `${origin}${context.match.pathname}`,
      },
    });
  },
  component: NewsPostPage,
  notFoundComponent: NewsPostNotFound,
});

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
      <RichTextBody html={post.html} />
    </main>
  );
}

function NewsPostNotFound() {
  return (
    <NotFound
      title="Berita tidak ditemukan"
      text="Berita ini mungkin sudah tidak diterbitkan atau alamatnya salah."
      list={{ to: '/berita', label: 'Lihat semua berita' }}
    />
  );
}
