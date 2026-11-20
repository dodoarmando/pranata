import { createFileRoute, Link, notFound } from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import { period, validityLabels, validityVariants } from '../../../announcement';
import { getPublishedAnnouncement, type Village } from '../../../api/generated';
import { RichTextBody } from '../../../components/rich-text-body';
import { Badge } from '../../../components/ui/badge';
import { villageTitle } from '../../../village';

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
        <Badge variant={validityVariants[announcement.validity]}>
          {validityLabels[announcement.validity]}
        </Badge>
        {period(announcement.startDate, announcement.endDate)}
      </p>
      <p className="text-lg font-medium">{announcement.summary}</p>
      <RichTextBody html={announcement.html} />
    </main>
  );
}

function AnnouncementNotFound() {
  return (
    <main className="site-container grid gap-4 pt-10 pb-sec md:pt-16">
      <h1 className="text-[2rem] leading-tight font-semibold tracking-tight">
        Pengumuman tidak ditemukan
      </h1>
      <p className="text-muted-foreground">
        Pengumuman ini mungkin sudah tidak diterbitkan, tidak punya halaman sendiri, atau alamatnya
        salah.
      </p>
      <p className="flex flex-wrap gap-4">
        <Link to="/pengumuman" className="font-medium text-primary">
          Lihat semua pengumuman
        </Link>
        <Link to="/" className="font-medium text-primary">
          Kembali ke beranda
        </Link>
      </p>
    </main>
  );
}
