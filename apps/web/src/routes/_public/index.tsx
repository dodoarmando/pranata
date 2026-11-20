import { createFileRoute, Link } from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import {
  type AnnouncementCard as Announcement,
  getHealth,
  type Health,
  listAnnouncementsInEffect,
} from '../../api/generated';
import { AnnouncementCard } from '../../components/announcement-card';

// Server-only so the browser never needs to reach the API directly.
const fetchHome = createServerFn({ method: 'GET' }).handler(
  async (): Promise<{ health: Health; announcements: Announcement[] }> => {
    try {
      const [health, announcements] = await Promise.all([getHealth(), listAnnouncementsInEffect()]);
      return {
        health: health.data,
        announcements: announcements.status === 200 ? announcements.data : [],
      };
    } catch {
      return { health: { status: 'unavailable' }, announcements: [] };
    }
  },
);

export const Route = createFileRoute('/_public/')({
  loader: () => fetchHome(),
  component: HomePage,
});

function HomePage() {
  const { health, announcements } = Route.useLoaderData();
  return (
    <main className="site-container grid gap-10 pt-10 pb-sec md:pt-16">
      <div className="grid gap-3">
        <h1 className="text-[2rem] leading-tight font-semibold tracking-tight md:text-[2.5rem] xl:text-5xl">
          Pranata
        </h1>
        <p className="text-muted-foreground md:text-lg">
          Status API: {health.status === 'ok' ? 'sehat' : 'tidak tersedia'}
        </p>
      </div>
      {announcements.length > 0 && (
        <section aria-labelledby="announcements" className="grid gap-4">
          <div className="flex flex-wrap items-baseline justify-between gap-2">
            <h2 id="announcements" className="text-2xl font-semibold tracking-tight">
              Pengumuman
            </h2>
            <Link to="/pengumuman" className="font-medium text-primary">
              Lihat semua pengumuman
            </Link>
          </div>
          <ul className="grid gap-4 md:grid-cols-2">
            {announcements.map((announcement) => (
              <li key={announcement.slug} className="grid">
                <AnnouncementCard announcement={announcement} heading="h3" />
              </li>
            ))}
          </ul>
        </section>
      )}
    </main>
  );
}
