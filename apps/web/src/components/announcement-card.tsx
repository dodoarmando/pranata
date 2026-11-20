import { Link } from '@tanstack/react-router';
import { period } from '../announcement';
import type { AnnouncementCard as Card, Validity } from '../api/generated';
import { Badge } from './ui/badge';

const validityLabels: Record<Validity, string> = {
  upcoming: 'Akan berlaku',
  inEffect: 'Berlaku',
  ended: 'Berakhir',
};

const validityVariants: Record<Validity, 'secondary' | 'default' | 'outline'> = {
  upcoming: 'secondary',
  inEffect: 'default',
  ended: 'outline',
};

/** Berlaku, Akan berlaku or Berakhir. */
export function ValidityBadge({ validity }: { validity: Validity }) {
  return <Badge variant={validityVariants[validity]}>{validityLabels[validity]}</Badge>;
}

/** A Published Announcement in a public list; "Selengkapnya" only when it has a body. */
export function AnnouncementCard({
  announcement,
  heading: Heading = 'h2',
}: {
  announcement: Card;
  /** Its title's level, under the page's or section's own heading. */
  heading?: 'h2' | 'h3';
}) {
  return (
    <article className="grid gap-2 rounded-md border bg-card p-5">
      <p className="flex flex-wrap items-center gap-2 text-sm text-muted-foreground">
        <ValidityBadge validity={announcement.validity} />
        {period(announcement.startDate, announcement.endDate)}
      </p>
      <Heading className="text-lg leading-snug font-semibold">{announcement.title}</Heading>
      <p className="text-muted-foreground">{announcement.summary}</p>
      {announcement.hasBody && (
        <Link
          to="/pengumuman/$slug"
          params={{ slug: announcement.slug }}
          className="w-fit font-medium text-primary"
        >
          Selengkapnya<span className="sr-only">: {announcement.title}</span>
        </Link>
      )}
    </article>
  );
}
