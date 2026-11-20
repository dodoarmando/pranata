import type { Timing } from '../api/generated';
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
