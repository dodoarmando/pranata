import { createFileRoute, getRouteApi, Link, useNavigate } from '@tanstack/react-router';
import { createServerFn, useServerFn } from '@tanstack/react-start';
import { Plus } from 'lucide-react';
import { type FormEvent, useState } from 'react';
import { createEvent, listEvents } from '../../../api/generated';
import { DataTable, Notice, PageHead } from '../../../components/admin';
import { TimingBadges } from '../../../components/event-card';
import { Badge } from '../../../components/ui/badge';
import { Button } from '../../../components/ui/button';
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '../../../components/ui/dialog';
import { Input } from '../../../components/ui/input';
import { Label } from '../../../components/ui/label';
import {
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '../../../components/ui/table';
import { schedule } from '../../../event';
import { ianaZones } from '../../../village';

// Server-only; the list is null without a content Permission.
const fetchEvents = createServerFn({ method: 'GET' }).handler(async () => {
  const response = await listEvents();
  return {
    events: response.status === 200 ? response.data : null,
    forbidden: response.status === 403,
  };
});

const createFn = createServerFn({ method: 'POST' })
  .validator((title: string) => title)
  .handler(async ({ data }) => {
    const response = await createEvent({ title: data });
    return response.status === 201
      ? { status: response.status, id: response.data.id }
      : { status: response.status };
  });

export const Route = createFileRoute('/admin/agenda/')({
  loader: () => fetchEvents(),
  head: () => ({ meta: [{ title: 'Agenda · Pranata' }] }),
  component: Events,
});

const root = getRouteApi('__root__');

function Events() {
  const { events, forbidden } = Route.useLoaderData();
  const village = root.useLoaderData();
  const navigate = useNavigate();
  const create = useServerFn(createFn);
  const [writing, setWriting] = useState(false);
  const [dialogError, setDialogError] = useState('');

  if (!events) {
    return (
      <>
        <PageHead title="Agenda" />
        <Notice error>
          {forbidden ? 'Anda tidak memiliki izin menulis konten.' : 'Gagal memuat agenda.'}
        </Notice>
      </>
    );
  }

  const zone = village?.timeZone ?? 'WIB';
  const date = new Intl.DateTimeFormat('id-ID', {
    dateStyle: 'medium',
    timeStyle: 'short',
    timeZone: ianaZones[zone],
  });

  async function onWrite(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const title = String(new FormData(event.currentTarget).get('title') ?? '').trim();
    const result = await create({ data: title });
    if (!result.id) {
      setDialogError(
        result.status === 403
          ? 'Anda tidak memiliki izin menulis konten.'
          : 'Judul wajib diisi, paling banyak 150 karakter.',
      );
      return;
    }
    await navigate({ to: '/admin/agenda/$id', params: { id: result.id } });
  }

  return (
    <>
      <PageHead title="Agenda">
        Kegiatan desa tampil di halaman Agenda: yang akan datang di atas, yang sudah lewat di
        bawahnya. Kegiatan yang batal tetap tercantum dengan label Dibatalkan.
      </PageHead>
      <Button
        type="button"
        size="lg"
        className="w-fit px-4"
        onClick={() => {
          setDialogError('');
          setWriting(true);
        }}
      >
        <Plus aria-hidden="true" />
        Tulis kegiatan
      </Button>
      {events.length === 0 ? (
        <p className="text-muted-foreground">Belum ada kegiatan.</p>
      ) : (
        <DataTable>
          <TableHeader>
            <TableRow>
              <TableHead>Judul</TableHead>
              <TableHead>Waktu</TableHead>
              <TableHead>Kategori</TableHead>
              <TableHead>Status</TableHead>
              <TableHead>Diubah</TableHead>
              <TableHead>Penulis</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {events.map((event) => (
              <TableRow key={event.id}>
                <TableCell className="max-w-96 truncate">
                  <Link
                    to="/admin/agenda/$id"
                    params={{ id: event.id }}
                    className="font-medium underline-offset-4 hover:underline"
                  >
                    {event.title}
                  </Link>
                </TableCell>
                <TableCell>
                  {event.startsAt ? (
                    <span className="flex flex-wrap items-center gap-2">
                      {schedule(event.startsAt, event.endsAt, zone)}
                      {event.timing && (
                        <TimingBadges timing={event.timing} cancelled={event.cancelled} />
                      )}
                    </span>
                  ) : (
                    '—'
                  )}
                </TableCell>
                <TableCell>{event.category ?? '—'}</TableCell>
                <TableCell>
                  <Badge variant={event.published ? 'default' : 'secondary'}>
                    {event.published ? 'Terbit' : 'Draf'}
                  </Badge>
                </TableCell>
                <TableCell>{date.format(new Date(event.updatedAt))}</TableCell>
                <TableCell>{event.author}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </DataTable>
      )}
      <Dialog open={writing} onOpenChange={setWriting}>
        <DialogContent>
          <form onSubmit={onWrite} className="grid gap-4">
            <DialogHeader>
              <DialogTitle>Tulis kegiatan</DialogTitle>
            </DialogHeader>
            <div className="grid gap-2">
              <Label htmlFor="new-event-title">Judul</Label>
              <Input id="new-event-title" name="title" required maxLength={150} />
            </div>
            {dialogError && <Notice error>{dialogError}</Notice>}
            <DialogFooter>
              <DialogClose
                render={<Button type="button" variant="outline" size="lg" className="px-4" />}
              >
                Batal
              </DialogClose>
              <Button type="submit" size="lg" className="px-4">
                Mulai menulis
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
    </>
  );
}
