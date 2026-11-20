import { createFileRoute, getRouteApi, Link, useNavigate } from '@tanstack/react-router';
import { createServerFn, useServerFn } from '@tanstack/react-start';
import { Plus } from 'lucide-react';
import { type FormEvent, useState } from 'react';
import { createPage, listPages, type PageMenu } from '../../../api/generated';
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
import { NativeSelect, NativeSelectOption } from '../../../components/ui/native-select';
import {
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '../../../components/ui/table';
import { pageMenus } from '../../../page';
import { ianaZones } from '../../../village';

// Server-only; the pages are null without a content Permission.
const fetchPages = createServerFn({ method: 'GET' }).handler(async () => {
  const pages = await listPages();
  return { pages: pages.status === 200 ? pages.data : null, forbidden: pages.status === 403 };
});

const createPageFn = createServerFn({ method: 'POST' })
  .validator((data: { title: string; menu: PageMenu }) => data)
  .handler(async ({ data }) => {
    const response = await createPage(data);
    return response.status === 201
      ? { status: response.status, id: response.data.id }
      : { status: response.status };
  });

export const Route = createFileRoute('/admin/halaman/')({
  loader: () => fetchPages(),
  head: () => ({ meta: [{ title: 'Halaman · Pranata' }] }),
  component: Pages,
});

const root = getRouteApi('__root__');

function Pages() {
  const { pages, forbidden } = Route.useLoaderData();
  const village = root.useLoaderData();
  const navigate = useNavigate();
  const create = useServerFn(createPageFn);
  const [writing, setWriting] = useState(false);
  const [dialogError, setDialogError] = useState('');

  if (!pages) {
    return (
      <>
        <PageHead title="Halaman" />
        <Notice error>
          {forbidden ? 'Anda tidak memiliki izin menulis konten.' : 'Gagal memuat halaman.'}
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
      data: {
        title: String(form.get('title') ?? '').trim(),
        menu: String(form.get('menu')) as PageMenu,
      },
    });
    if (!result.id) {
      setDialogError(
        result.status === 403
          ? 'Anda tidak memiliki izin menulis konten.'
          : 'Judul wajib diisi, paling banyak 150 karakter.',
      );
      return;
    }
    await navigate({ to: '/admin/halaman/$id', params: { id: result.id } });
  }

  return (
    <>
      <PageHead title="Halaman">
        Halaman tampil di menu Profil, Pemerintahan, atau Layanan setelah diterbitkan, berurutan
        dari nomor urut terkecil.
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
        Tulis halaman
      </Button>
      {pages.length === 0 ? (
        <p className="text-muted-foreground">Belum ada halaman.</p>
      ) : (
        <DataTable>
          <TableHeader>
            <TableRow>
              <TableHead>Menu</TableHead>
              <TableHead>Urutan</TableHead>
              <TableHead>Judul</TableHead>
              <TableHead>Status</TableHead>
              <TableHead>Diubah</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {pages.map((page) => (
              <TableRow key={page.id}>
                <TableCell>{pageMenus[page.menu].label}</TableCell>
                <TableCell>{page.order}</TableCell>
                <TableCell className="max-w-96 truncate">
                  <Link
                    to="/admin/halaman/$id"
                    params={{ id: page.id }}
                    className="font-medium underline-offset-4 hover:underline"
                  >
                    {page.title}
                  </Link>
                </TableCell>
                <TableCell>
                  <Badge variant={page.published ? 'default' : 'secondary'}>
                    {page.published ? 'Terbit' : 'Draf'}
                  </Badge>
                </TableCell>
                <TableCell>{date.format(new Date(page.updatedAt))}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </DataTable>
      )}
      <Dialog open={writing} onOpenChange={setWriting}>
        <DialogContent>
          <form onSubmit={onWrite} className="grid gap-4">
            <DialogHeader>
              <DialogTitle>Tulis halaman</DialogTitle>
            </DialogHeader>
            <div className="grid gap-2">
              <Label htmlFor="new-page-title">Judul</Label>
              <Input id="new-page-title" name="title" required maxLength={150} />
            </div>
            <div className="grid gap-2">
              <Label htmlFor="new-page-menu">Menu</Label>
              <NativeSelect id="new-page-menu" name="menu">
                {Object.entries(pageMenus).map(([menu, { label }]) => (
                  <NativeSelectOption key={menu} value={menu}>
                    {label}
                  </NativeSelectOption>
                ))}
              </NativeSelect>
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
