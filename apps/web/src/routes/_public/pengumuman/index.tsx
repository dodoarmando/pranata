import { createFileRoute, Link } from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import { type AnnouncementPage, listPublishedAnnouncements } from '../../../api/generated';
import { AnnouncementCard } from '../../../components/announcement-card';
import { pageHead } from '../../../seo';

type Search = { halaman?: number };

// Server-only; empty when the API cannot be reached.
const fetchAnnouncements = createServerFn({ method: 'GET' })
  .validator((search: Search) => search)
  .handler(async ({ data }): Promise<AnnouncementPage> => {
    const empty = { announcements: [], total: 0, pageSize: 10 };
    try {
      const response = await listPublishedAnnouncements(data.halaman ? { page: data.halaman } : {});
      return response.status === 200 ? response.data : empty;
    } catch {
      return empty;
    }
  });

export const Route = createFileRoute('/_public/pengumuman/')({
  validateSearch: (search: Record<string, unknown>): Search => {
    const page = Number(search.halaman);
    return Number.isInteger(page) && page > 1 ? { halaman: page } : {};
  },
  loaderDeps: ({ search }) => search,
  loader: ({ deps }) => fetchAnnouncements({ data: deps }),
  head: (context) => pageHead(context, { title: 'Pengumuman' }),
  component: AnnouncementList,
});

function AnnouncementList() {
  const page = Route.useLoaderData();
  const current = Route.useSearch().halaman ?? 1;
  const pages = Math.max(1, Math.ceil(page.total / page.pageSize));
  return (
    <main className="site-container grid max-w-3xl gap-8 pt-10 pb-sec md:pt-16">
      <h1 className="text-[2rem] leading-tight font-semibold tracking-tight md:text-[2.5rem]">
        Pengumuman
      </h1>
      {page.announcements.length ? (
        <ul className="grid gap-4">
          {page.announcements.map((announcement) => (
            <li key={announcement.slug}>
              <AnnouncementCard announcement={announcement} />
            </li>
          ))}
        </ul>
      ) : (
        <p className="text-muted-foreground">Belum ada pengumuman.</p>
      )}
      {pages > 1 && (
        <nav aria-label="Halaman pengumuman" className="flex items-center justify-between gap-4">
          {current > 1 ? (
            <Link
              to="/pengumuman"
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
              to="/pengumuman"
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
