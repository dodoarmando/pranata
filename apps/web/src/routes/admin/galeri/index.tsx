import { createFileRoute, getRouteApi, Link, useNavigate } from '@tanstack/react-router';
import { createServerFn, useServerFn } from '@tanstack/react-start';
import { Plus } from 'lucide-react';
import { type FormEvent, useState } from 'react';
import { calendarDate } from '../../../announcement';
import { createAlbum, listAlbums } from '../../../api/generated';
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

// Server-only; the albums are null without a content Permission.
const fetchAlbums = createServerFn({ method: 'GET' }).handler(async () => {
  const albums = await listAlbums();
  return { albums: albums.status === 200 ? albums.data : null, forbidden: albums.status === 403 };
});

const createAlbumFn = createServerFn({ method: 'POST' })
  .validator((data: { title: string }) => data)
  .handler(async ({ data }) => {
    const response = await createAlbum(data);
    return response.status === 201
      ? { status: response.status, id: response.data.id }
      : { status: response.status };
  });

export const Route = createFileRoute('/admin/galeri/')({
  loader: () => fetchAlbums(),
  head: () => ({ meta: [{ title: 'Galeri · Pranata' }] }),
  component: Albums,
});

const root = getRouteApi('__root__');

function Albums() {
  const { albums, forbidden } = Route.useLoaderData();
  const village = root.useLoaderData();
  const navigate = useNavigate();
  const create = useServerFn(createAlbumFn);
  const [writing, setWriting] = useState(false);
  const [dialogError, setDialogError] = useState('');

  if (!albums) {
    return (
      <>
        <PageHead title="Galeri" />
        <Notice error>
          {forbidden ? 'Anda tidak memiliki izin menulis konten.' : 'Gagal memuat galeri.'}
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
    const form = new FormData(event.currentTarget);
    const result = await create({
      data: { title: String(form.get('title') ?? '').trim() },
    });
    if (!result.id) {
      setDialogError(
        result.status === 403
          ? 'Anda tidak memiliki izin menulis konten.'
          : 'Judul wajib diisi, paling banyak 150 karakter.',
      );
      return;
    }
    await navigate({ to: '/admin/galeri/$id', params: { id: result.id } });
  }

  return (
    <>
      <PageHead title="Galeri">
        Album foto kegiatan desa tampil di halaman Galeri setelah diterbitkan, dari kegiatan
        terbaru.
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
        Buat album
      </Button>
      {albums.length === 0 ? (
        <p className="text-muted-foreground">Belum ada album.</p>
      ) : (
        <DataTable>
          <TableHeader>
            <TableRow>
              <TableHead>Tanggal kegiatan</TableHead>
              <TableHead>Judul</TableHead>
              <TableHead>Foto</TableHead>
              <TableHead>Status</TableHead>
              <TableHead>Diubah</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {albums.map((album) => (
              <TableRow key={album.id}>
                <TableCell>
                  {album.occasionStart ? calendarDate(album.occasionStart) : '—'}
                </TableCell>
                <TableCell className="max-w-96 truncate">
                  <Link
                    to="/admin/galeri/$id"
                    params={{ id: album.id }}
                    className="font-medium underline-offset-4 hover:underline"
                  >
                    {album.title}
                  </Link>
                </TableCell>
                <TableCell>{album.photoCount}</TableCell>
                <TableCell>
                  <Badge variant={album.published ? 'default' : 'secondary'}>
                    {album.published ? 'Terbit' : 'Draf'}
                  </Badge>
                </TableCell>
                <TableCell>{date.format(new Date(album.updatedAt))}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </DataTable>
      )}
      <Dialog open={writing} onOpenChange={setWriting}>
        <DialogContent>
          <form onSubmit={onWrite} className="grid gap-4">
            <DialogHeader>
              <DialogTitle>Buat album</DialogTitle>
            </DialogHeader>
            <div className="grid gap-2">
              <Label htmlFor="new-album-title">Judul</Label>
              <Input id="new-album-title" name="title" required maxLength={150} />
            </div>
            {dialogError && <Notice error>{dialogError}</Notice>}
            <DialogFooter>
              <DialogClose
                render={<Button type="button" variant="outline" size="lg" className="px-4" />}
              >
                Batal
              </DialogClose>
              <Button type="submit" size="lg" className="px-4">
                Mulai mengisi
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
    </>
  );
}
