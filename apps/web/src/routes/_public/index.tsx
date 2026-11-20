import { createFileRoute, Link } from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import {
  type AnnouncementCard as Announcement,
  getHealth,
  type Health,
  listAnnouncementsInEffect,
  listPublishedPartners,
  type PublishedPartner,
} from '../../api/generated';
import { AnnouncementCard } from '../../components/announcement-card';

// Server-only so the browser never needs to reach the API directly.
const fetchHome = createServerFn({ method: 'GET' }).handler(
  async (): Promise<{
    health: Health;
    announcements: Announcement[];
    partners: PublishedPartner[];
  }> => {
    try {
      const [health, announcements, partners] = await Promise.all([
        getHealth(),
        listAnnouncementsInEffect(),
        listPublishedPartners(),
      ]);
      return {
        health: health.data,
        announcements: announcements.status === 200 ? announcements.data : [],
        partners: partners.status === 200 ? partners.data : [],
      };
    } catch {
      return { health: { status: 'unavailable' }, announcements: [], partners: [] };
    }
  },
);

export const Route = createFileRoute('/_public/')({
  loader: () => fetchHome(),
  component: HomePage,
});

function HomePage() {
  const { health, announcements, partners } = Route.useLoaderData();
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
      {partners.length > 0 && (
        <section aria-labelledby="partners" className="grid gap-4">
          <h2 id="partners" className="text-2xl font-semibold tracking-tight">
            Mitra desa
          </h2>
          <ul className="flex flex-wrap items-center gap-x-10 gap-y-6">
            {partners.map((partner, index) => {
              const logo = (
                <img
                  src={`/media/${partner.logo}/480`}
                  alt={partner.name}
                  width={160}
                  height={64}
                  loading="lazy"
                  className="h-16 w-40 object-contain"
                />
              );
              return (
                // A static list the server orders; a place is unique where logos may repeat.
                // biome-ignore lint/suspicious/noArrayIndexKey: see above
                <li key={index}>
                  {partner.url ? (
                    <a href={partner.url} target="_blank" rel="noopener noreferrer">
                      {logo}
                    </a>
                  ) : (
                    logo
                  )}
                </li>
              );
            })}
          </ul>
        </section>
      )}
    </main>
  );
}
