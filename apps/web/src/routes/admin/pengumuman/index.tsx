import { createFileRoute, getRouteApi, Link, useNavigate } from '@tanstack/react-router';
import { createServerFn, useServerFn } from '@tanstack/react-start';
import { Plus } from 'lucide-react';
import { type FormEvent, useState } from 'react';
import { period, validityLabels, validityVariants } from '../../../announcement';
import { createAnnouncement, listAnnouncements } from '../../../api/generated';
import { DataTable, Notice, PageHead } from '../../../components/admin';
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
import { ianaZones } from '../../../village';

// Server-only; the list is null without a content Permission.
const fetchAnnouncements = createServerFn({ method: 'GET' }).handler(async () => {
  const response = await listAnnouncements();
  return {
    announcements: response.status === 200 ? response.data : null,
    forbidden: response.status === 403,
  };
});

const createFn = createServerFn({ method: 'POST' })
  .validator((title: string) => title)
  .handler(async ({ data }) => {
    const response = await createAnnouncement({ title: data });
    return response.status === 201
      ? { status: response.status, id: response.data.id }
      : { status: response.status };
  });

export const Route = createFileRoute('/admin/pengumuman/')({
  loader: () => fetchAnnouncements(),
  head: () => ({ meta: [{ title: 'Pengumuman · Pranata' }] }),
  component: Announcements,
});

const root = getRouteApi('__root__');

function Announcements() {
  const { announcements, forbidden } = Route.useLoaderData();
  const village = root.useLoaderData();
  const navigate = useNavigate();
  const create = useServerFn(createFn);
  const [writing, setWriting] = useState(false);
  const [dialogError, setDialogError] = useState('');

  if (!announcements) {
    return (
      <>
        <PageHead title="Pengumuman" />
        <Notice error>
          {forbidden ? 'Anda tidak memiliki izin menulis konten.' : 'Gagal memuat pengumuman.'}
        </Notice>
      </>
    );
  }

  const date = new Intl.DateTimeFormat('id-ID', {
    dateStyle: 'medium',
    timeStyle: 'short',
    timeZone: ianaZones[village?.timeZone ?? 'WIB'],
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
    await navigate({ to: '/admin/pengumuman/$id', params: { id: result.id } });
  }

  return (
    <>
      <PageHead title="Pengumuman">
        Pengumuman tampil di beranda selama masa berlakunya, dan tetap tercantum di halaman
        Pengumuman setelah berakhir.
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
        Tulis pengumuman
      </Button>
      {announcements.length === 0 ? (
        <p className="text-muted-foreground">Belum ada pengumuman.</p>
      ) : (
        <DataTable>
          <TableHeader>
            <TableRow>
              <TableHead>Judul</TableHead>
              <TableHead>Masa berlaku</TableHead>
              <TableHead>Status</TableHead>
              <TableHead>Diubah</TableHead>
              <TableHead>Penulis</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {announcements.map((announcement) => (
              <TableRow key={announcement.id}>
                <TableCell className="max-w-96 truncate">
                  <Link
                    to="/admin/pengumuman/$id"
                    params={{ id: announcement.id }}
                    className="font-medium underline-offset-4 hover:underline"
                  >
                    {announcement.title}
                  </Link>
                </TableCell>
                <TableCell>
                  {announcement.startDate && announcement.endDate ? (
                    <span className="flex flex-wrap items-center gap-2">
                      {period(announcement.startDate, announcement.endDate)}
                      {announcement.validity && (
                        <Badge variant={validityVariants[announcement.validity]}>
                          {validityLabels[announcement.validity]}
                        </Badge>
                      )}
                    </span>
                  ) : (
                    '—'
                  )}
                </TableCell>
                <TableCell>
                  <Badge variant={announcement.published ? 'default' : 'secondary'}>
                    {announcement.published ? 'Terbit' : 'Draf'}
                  </Badge>
                </TableCell>
                <TableCell>{date.format(new Date(announcement.updatedAt))}</TableCell>
                <TableCell>{announcement.author}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </DataTable>
      )}
      <Dialog open={writing} onOpenChange={setWriting}>
        <DialogContent>
          <form onSubmit={onWrite} className="grid gap-4">
            <DialogHeader>
              <DialogTitle>Tulis pengumuman</DialogTitle>
            </DialogHeader>
            <div className="grid gap-2">
              <Label htmlFor="new-announcement-title">Judul</Label>
              <Input id="new-announcement-title" name="title" required maxLength={150} />
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
