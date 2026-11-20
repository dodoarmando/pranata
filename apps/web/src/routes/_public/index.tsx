import { createFileRoute, getRouteApi, Link } from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import { Search } from 'lucide-react';
import type { ReactNode } from 'react';
import { calendarDate } from '../../announcement';
import { netFinancing, rupiah, share, spending, versionLabels } from '../../apbdes-summary';
import {
  type AnnouncementCard as Announcement,
  type EventCard as Event,
  getHomeHero,
  getHomeWelcome,
  getPublishedApbdesSummary,
  type HomeHero,
  type HomeWelcome,
  listAnnouncementsInEffect,
  listPublishedNewsPosts,
  listPublishedPartners,
  listUpcomingEvents,
  type NewsCard,
  type PublishedApbdesSummary,
  type PublishedPartner,
} from '../../api/generated';
import { AnnouncementCard } from '../../components/announcement-card';
import { EventCard } from '../../components/event-card';
import { Button } from '../../components/ui/button';
import { holderTitle } from '../../government';
import { newsDate } from '../../news';
import { villageTitle } from '../../village';

type Home = {
  hero: HomeHero | null;
  welcome: HomeWelcome | null;
  news: NewsCard[];
  announcements: Announcement[];
  events: Event[];
  apbdes: PublishedApbdesSummary | null;
  partners: PublishedPartner[];
};

// Server-only so the browser never needs to reach the API directly; a section the API cannot
// give is left out rather than failing the page.
const fetchHome = createServerFn({ method: 'GET' }).handler(async (): Promise<Home> => {
  const ok = <T,>(response: PromiseSettledResult<{ status: number; data: unknown }>) =>
    response.status === 'fulfilled' && response.value.status === 200
      ? (response.value.data as T)
      : null;
  const [hero, welcome, news, announcements, events, apbdes, partners] = await Promise.allSettled([
    getHomeHero(),
    getHomeWelcome(),
    listPublishedNewsPosts(),
    listAnnouncementsInEffect(),
    listUpcomingEvents(),
    getPublishedApbdesSummary(),
    listPublishedPartners(),
  ]);
  return {
    hero: ok<HomeHero>(hero),
    welcome: ok<HomeWelcome>(welcome),
    news: ok<{ posts: NewsCard[] }>(news)?.posts.slice(0, 4) ?? [],
    announcements: ok<Announcement[]>(announcements) ?? [],
    events: ok<Event[]>(events)?.slice(0, 3) ?? [],
    apbdes: ok<PublishedApbdesSummary>(apbdes),
    partners: ok<PublishedPartner[]>(partners) ?? [],
  };
});

const root = getRouteApi('__root__');

export const Route = createFileRoute('/_public/')({
  loader: () => fetchHome(),
  component: HomePage,
});

/** A home page section with its heading and a link to the full list. */
function Section({
  id,
  title,
  more,
  muted = false,
  children,
}: {
  id: string;
  title: string;
  more?: { to: string; label: string };
  muted?: boolean;
  children: ReactNode;
}) {
  return (
    <section aria-labelledby={id} className={muted ? 'bg-muted' : undefined}>
      <div className="site-container grid gap-6 py-sec">
        <div className="flex flex-wrap items-baseline justify-between gap-2">
          <h2 id={id} className="text-2xl font-semibold tracking-tight md:text-[1.75rem]">
            {title}
          </h2>
          {more && (
            <Link to={more.to} className="font-medium text-primary">
              {more.label}
            </Link>
          )}
        </div>
        {children}
      </div>
    </section>
  );
}

function HomePage() {
  const village = root.useLoaderData();
  const { hero, welcome, news, announcements, events, apbdes, partners } = Route.useLoaderData();
  const zone = village?.timeZone ?? 'WIB';
  const [lead, ...rest] = news;
  return (
    <main>
      <section aria-labelledby="hero-title" className="relative isolate bg-pri-dd text-white">
        {hero && (
          <img
            src={`/media/${hero.photo}/1600`}
            srcSet={`/media/${hero.photo}/960 960w, /media/${hero.photo}/1600 1600w, /media/${hero.photo}/2400 2400w`}
            sizes="100vw"
            alt={hero.photoAlt}
            width={2400}
            height={1200}
            fetchPriority="high"
            className="absolute inset-0 -z-10 size-full object-cover"
          />
        )}
        <div className="absolute inset-0 -z-10 bg-linear-to-t from-pri-dd/90 via-pri-dd/60 to-pri-dd/30" />
        <div className="site-container grid max-w-3xl gap-5 py-20 md:py-28">
          <h1
            id="hero-title"
            className="text-[2rem] leading-tight font-semibold tracking-tight md:text-[2.75rem] xl:text-5xl"
          >
            {hero?.title ?? (village ? villageTitle(village) : 'Pranata')}
          </h1>
          {hero?.subtitle && <p className="text-on-dark md:text-lg">{hero.subtitle}</p>}
          {hero?.buttonLabel && hero.buttonPath && (
            <a
              href={hero.buttonPath}
              className="w-fit rounded-md bg-white px-4 py-2 font-medium text-pri-dd hover:bg-pri-50"
            >
              {hero.buttonLabel}
            </a>
          )}
          <search>
            <form action="/cari" method="get" className="flex gap-2">
              <label className="flex flex-1 items-center gap-2 rounded-md bg-white px-3 text-foreground">
                <Search aria-hidden="true" className="size-4 text-muted-foreground" />
                <span className="sr-only">Cari</span>
                <input
                  type="search"
                  name="q"
                  required
                  minLength={2}
                  maxLength={100}
                  placeholder="Cari berita, pengumuman, atau dokumen…"
                  className="h-11 w-full bg-transparent outline-none"
                />
              </label>
              <Button type="submit" size="lg" className="h-11 px-5">
                Cari
              </Button>
            </form>
          </search>
          {!!hero?.quickLinks?.length && (
            <nav aria-label="Tautan cepat" className="flex flex-wrap items-center gap-2 text-sm">
              {hero.quickLinks.map((link) => (
                <a
                  key={`${link.label}${link.path}`}
                  href={link.path}
                  className="rounded-full border border-white/40 px-3 py-1 hover:bg-white/10"
                >
                  {link.label}
                </a>
              ))}
            </nav>
          )}
        </div>
      </section>
      {announcements.length > 0 && (
        <Section
          id="announcements"
          title="Pengumuman"
          more={{ to: '/pengumuman', label: 'Lihat semua pengumuman' }}
        >
          <ul className="grid gap-4 md:grid-cols-2">
            {announcements.map((announcement) => (
              <li key={announcement.slug} className="grid">
                <AnnouncementCard announcement={announcement} heading="h3" />
              </li>
            ))}
          </ul>
        </Section>
      )}
      {lead && (
        <Section
          id="news"
          title="Berita terkini"
          more={{ to: '/berita', label: 'Semua berita' }}
          muted
        >
          <div className="grid gap-6 lg:grid-cols-[3fr_2fr]">
            <article className="overflow-hidden rounded-md border bg-card">
              <Link to="/berita/$slug" params={{ slug: lead.slug }} className="group grid">
                <img
                  src={`/media/${lead.cover}/960`}
                  srcSet={`/media/${lead.cover}/480 480w, /media/${lead.cover}/960 960w`}
                  sizes="(min-width: 1024px) 40rem, 100vw"
                  alt={lead.coverAlt}
                  width={960}
                  height={540}
                  loading="lazy"
                  className="aspect-video w-full object-cover"
                />
                <div className="grid gap-2 p-5">
                  <p className="text-sm text-muted-foreground">
                    {lead.category} · {newsDate(lead.publishedAt, zone)}
                  </p>
                  <h3 className="text-xl leading-snug font-semibold group-hover:text-primary">
                    {lead.title}
                  </h3>
                  <p className="line-clamp-3 text-muted-foreground">{lead.summary}</p>
                </div>
              </Link>
            </article>
            {rest.length > 0 && (
              <ul className="grid content-start gap-4">
                {rest.map((post) => (
                  <li key={post.slug}>
                    <Link
                      to="/berita/$slug"
                      params={{ slug: post.slug }}
                      className="group grid grid-cols-[7rem_1fr] gap-4 overflow-hidden rounded-md border bg-card"
                    >
                      <img
                        src={`/media/${post.cover}/480`}
                        alt={post.coverAlt}
                        width={480}
                        height={270}
                        sizes="7rem"
                        loading="lazy"
                        className="h-full w-full object-cover"
                      />
                      <div className="grid content-center gap-1 py-3 pr-4">
                        <h3 className="leading-snug font-semibold group-hover:text-primary">
                          {post.title}
                        </h3>
                        <p className="text-sm text-muted-foreground">
                          {newsDate(post.publishedAt, zone)}
                        </p>
                      </div>
                    </Link>
                  </li>
                ))}
              </ul>
            )}
          </div>
        </Section>
      )}
      {events.length > 0 && (
        <Section
          id="events"
          title="Agenda kegiatan"
          more={{ to: '/agenda', label: 'Semua agenda' }}
        >
          <ul className="grid gap-4 lg:grid-cols-3">
            {events.map((event) => (
              <li key={event.id} className="grid">
                <EventCard event={event} zone={zone} />
              </li>
            ))}
          </ul>
        </Section>
      )}
      {apbdes && <ApbdesSection summary={apbdes} />}
      {welcome && <WelcomeSection welcome={welcome} />}
      {partners.length > 0 && (
        <Section id="partners" title="Mitra desa">
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
        </Section>
      )}
    </main>
  );
}

/** The latest Published APBDes Summary in three figures. */
function ApbdesSection({ summary }: { summary: PublishedApbdesSummary }) {
  const { budget, realisation } = summary;
  const stats = [
    {
      label: 'Pendapatan',
      amount: budget.revenue,
      note: realisation && `Realisasi ${share(realisation.revenue, budget.revenue)}%`,
    },
    {
      label: 'Belanja',
      amount: spending(budget),
      note: realisation && `Realisasi ${share(spending(realisation), spending(budget))}%`,
    },
    { label: 'Pembiayaan netto', amount: netFinancing(budget), note: null },
  ];
  return (
    <Section
      id="apbdes"
      title={`APBDes Tahun Anggaran ${summary.fiscalYear}`}
      more={{ to: '/apbdes', label: 'Lihat rincian APBDes' }}
      muted
    >
      <p className="text-muted-foreground">
        Versi {versionLabels[summary.version]}, ditetapkan {calendarDate(summary.versionDate)}.
        {realisation && ` Realisasi per ${calendarDate(realisation.asOf)}.`}
      </p>
      <dl className="grid gap-4 md:grid-cols-3">
        {stats.map((stat) => (
          <div key={stat.label} className="grid gap-1 rounded-md border bg-card p-5">
            <dt className="text-muted-foreground">{stat.label}</dt>
            <dd className="text-xl font-semibold tabular-nums">{rupiah(stat.amount)}</dd>
            {stat.note && <dd className="text-sm text-muted-foreground">{stat.note}</dd>}
          </div>
        ))}
      </dl>
    </Section>
  );
}

/** The head-of-village welcome; blank lines in the message separate its paragraphs. */
function WelcomeSection({ welcome }: { welcome: HomeWelcome }) {
  return (
    <Section
      id="welcome"
      title="Sambutan kepala desa"
      more={{ to: '/pemerintahan', label: 'Struktur pemerintahan' }}
    >
      <figure className="grid items-start gap-6 md:grid-cols-[12rem_1fr]">
        <img
          src={`/media/${welcome.photo}/480`}
          alt={welcome.photoAlt}
          width={480}
          height={640}
          sizes="12rem"
          loading="lazy"
          className="aspect-3/4 w-40 rounded-md object-cover md:w-48"
        />
        <div className="grid gap-4">
          <blockquote className="grid max-w-prose gap-3 text-lg">
            {welcome.message.split(/\n\s*\n/).map((paragraph, index) => (
              // Static text in its saved order; paragraphs may repeat.
              // biome-ignore lint/suspicious/noArrayIndexKey: see above
              <p key={index} className="whitespace-pre-line">
                {paragraph}
              </p>
            ))}
          </blockquote>
          <figcaption className="grid">
            <strong>{welcome.signerName}</strong>
            <span className="text-muted-foreground">
              {holderTitle(welcome.signerKind, welcome.signerPosition)}
            </span>
          </figcaption>
        </div>
      </figure>
    </Section>
  );
}
