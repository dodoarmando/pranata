import type { EventCard as Card, TimeZone, Timing } from '../api/generated';
import { schedule } from '../event';
import { Badge } from './ui/badge';

/** Akan datang or Selesai, and Dibatalkan for an Event called off. */
export function TimingBadges({ timing, cancelled }: { timing: Timing; cancelled: boolean }) {
  return (
    <>
      <Badge variant={timing === 'upcoming' ? 'default' : 'outline'}>
        {timing === 'upcoming' ? 'Akan datang' : 'Selesai'}
      </Badge>
      {cancelled && <Badge variant="destructive">Dibatalkan</Badge>}
    </>
  );
}

/** A Published Event on the public Agenda; it has no detail page. */
export function EventCard({ event, zone }: { event: Card; zone: TimeZone }) {
  return (
    <article className="grid gap-2 rounded-md border bg-card p-5">
      <p className="flex flex-wrap items-center gap-2 text-sm text-muted-foreground">
        {event.cancelled && <Badge variant="destructive">Dibatalkan</Badge>}
        <Badge variant="secondary">{event.category}</Badge>
      </p>
      <h3 className="text-lg leading-snug font-semibold">{event.title}</h3>
      <dl className="grid gap-1 text-sm">
        <div className="flex gap-2">
          <dt className="sr-only">Waktu</dt>
          <dd>{schedule(event.startsAt, event.endsAt, zone)}</dd>
        </div>
        <div className="flex gap-2">
          <dt className="sr-only">Tempat</dt>
          <dd>{event.location}</dd>
        </div>
      </dl>
      {event.description && (
        <p className="whitespace-pre-line text-muted-foreground">{event.description}</p>
      )}
    </article>
  );
}
