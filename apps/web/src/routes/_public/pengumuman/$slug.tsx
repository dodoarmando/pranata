import { createFileRoute, Link, notFound } from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import { period } from '../../../announcement';
import { getPublishedAnnouncement } from '../../../api/generated';
import { ValidityBadge } from '../../../components/announcement-card';
import { NotFound } from '../../../components/not-found';
import { RichTextBody } from '../../../components/rich-text-body';
import { pageHead } from '../../../seo';

// Server-only: the body becomes HTML here, so Tiptap never ships to the browser. Null for a
// slug no Published Announcement with a body has; any other failure throws.
const fetchAnnouncement = createServerFn({ method: 'GET' })
  .validator((slug: string) => slug)
  .handler(async ({ data }) => {
    const response = await getPublishedAnnouncement(data);
    if (response.status === 404) return null;
    if (response.status !== 200) throw new Error('The API could not load the Announcement');
    const { renderRichText } = await import('../../../rich-text-html');
    const { body, ...announcement } = response.data;
    return { ...announcement, html: renderRichText(body) };
  });

export const Route = createFileRoute('/_public/pengumuman/$slug')({
  loader: async ({ params }) => {
    const announcement = await fetchAnnouncement({ data: params.slug });
    if (!announcement) throw notFound();
    return announcement;
  },
  head: (context) =>
    context.loaderData
      ? pageHead(context, {
          title: context.loaderData.title,
          description: context.loaderData.summary,
        })
      : {},
  component: AnnouncementPage,
  notFoundComponent: AnnouncementNotFound,
});

function AnnouncementPage() {
  const announcement = Route.useLoaderData();
  return (
    <main className="site-container grid max-w-3xl gap-6 pt-10 pb-sec md:pt-16">
      <Link to="/pengumuman" className="w-fit font-medium text-primary">
        Pengumuman
      </Link>
      <h1 className="text-[2rem] leading-tight font-semibold tracking-tight md:text-[2.5rem]">
        {announcement.title}
      </h1>
      <p className="flex flex-wrap items-center gap-2 text-muted-foreground">
        <ValidityBadge validity={announcement.validity} />
        {period(announcement.startDate, announcement.endDate)}
      </p>
      <p className="text-lg font-medium">{announcement.summary}</p>
      <RichTextBody html={announcement.html} />
    </main>
  );
}

function AnnouncementNotFound() {
  return (
    <NotFound
      title="Pengumuman tidak ditemukan"
      text="Pengumuman ini mungkin sudah tidak diterbitkan, tidak punya halaman sendiri, atau alamatnya salah."
      list={{ to: '/pengumuman', label: 'Lihat semua pengumuman' }}
    />
  );
}
