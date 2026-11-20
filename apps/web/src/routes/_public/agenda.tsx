import { createFileRoute, getRouteApi, Link } from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import {
  type EventCard as Card,
  type EventPage,
  listPastEvents,
  listUpcomingEvents,
} from '../../api/generated';
import { EventCard } from '../../components/event-card';
import { pageHead } from '../../seo';

type Search = { halaman?: number };

// Server-only; empty when the API cannot be reached. Upcoming Events show on the first page.
const fetchEvents = createServerFn({ method: 'GET' })
  .validator((search: Search) => search)
  .handler(async ({ data }): Promise<{ upcoming: Card[]; past: EventPage }> => {
    const empty = { upcoming: [], past: { events: [], total: 0, pageSize: 10 } };
    try {
      const [upcoming, past] = await Promise.all([
        data.halaman ? null : listUpcomingEvents(),
        listPastEvents(data.halaman ? { page: data.halaman } : {}),
      ]);
      return {
        upcoming: upcoming?.status === 200 ? upcoming.data : [],
        past: past.status === 200 ? past.data : empty.past,
      };
    } catch {
      return empty;
    }
  });

export const Route = createFileRoute('/_public/agenda')({
  validateSearch: (search: Record<string, unknown>): Search => {
    const page = Number(search.halaman);
    return Number.isInteger(page) && page > 1 ? { halaman: page } : {};
  },
  loaderDeps: ({ search }) => search,
  loader: ({ deps }) => fetchEvents({ data: deps }),
  head: (context) => pageHead(context, { title: 'Agenda' }),
  component: EventList,
});

const root = getRouteApi('__root__');

function EventList() {
  const { upcoming, past } = Route.useLoaderData();
  const zone = root.useLoaderData()?.timeZone ?? 'WIB';
  const current = Route.useSearch().halaman ?? 1;
  const pages = Math.max(1, Math.ceil(past.total / past.pageSize));
  const list = (events: Card[]) => (
    <ul className="grid gap-4">
      {events.map((event) => (
        <li key={event.id}>
          <EventCard event={event} zone={zone} />
        </li>
      ))}
    </ul>
  );
  return (
    <main className="site-container grid max-w-3xl gap-8 pt-10 pb-sec md:pt-16">
      <h1 className="text-[2rem] leading-tight font-semibold tracking-tight md:text-[2.5rem]">
        Agenda
      </h1>
      {current === 1 && (
        <section aria-labelledby="upcoming" className="grid gap-4">
          <h2 id="upcoming" className="text-xl font-semibold">
            Akan datang
          </h2>
          {upcoming.length ? (
            list(upcoming)
          ) : (
            <p className="text-muted-foreground">Belum ada kegiatan yang akan datang.</p>
          )}
        </section>
      )}
      <section aria-labelledby="past" className="grid gap-4">
        <h2 id="past" className="text-xl font-semibold">
          Sudah lewat
        </h2>
        {past.events.length ? (
          list(past.events)
        ) : (
          <p className="text-muted-foreground">Belum ada kegiatan yang sudah lewat.</p>
        )}
      </section>
      {pages > 1 && (
        <nav aria-label="Halaman agenda" className="flex items-center justify-between gap-4">
          {current > 1 ? (
            <Link
              to="/agenda"
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
              to="/agenda"
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
