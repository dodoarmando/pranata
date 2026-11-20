import { createFileRoute, getRouteApi, Link, useNavigate, useRouter } from '@tanstack/react-router';
import { createServerFn, useServerFn } from '@tanstack/react-start';
import { Plus } from 'lucide-react';
import { type FormEvent, useState } from 'react';
import {
  type Category,
  createCategory,
  createNewsPost,
  deleteCategory,
  listCategories,
  listNewsPosts,
  updateCategory,
} from '../../../api/generated';
import { DataTable, Notice, PageHead } from '../../../components/admin';
import { Badge } from '../../../components/ui/badge';
import { Button } from '../../../components/ui/button';
import { Card, CardContent, CardHeader } from '../../../components/ui/card';
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

// Server-only; the posts are null without a content Permission.
const fetchNews = createServerFn({ method: 'GET' }).handler(async () => {
  const [posts, categories] = await Promise.all([listNewsPosts(), listCategories()]);
  return {
    posts: posts.status === 200 ? posts.data : null,
    forbidden: posts.status === 403,
    categories: categories.status === 200 ? categories.data : [],
  };
});

const createPostFn = createServerFn({ method: 'POST' })
  .validator((title: string) => title)
  .handler(async ({ data }) => {
    const response = await createNewsPost({ title: data });
    return response.status === 201
      ? { status: response.status, id: response.data.id }
      : { status: response.status };
  });

const createCategoryFn = createServerFn({ method: 'POST' })
  .validator((name: string) => name)
  .handler(async ({ data }) => (await createCategory({ name: data })).status);

const renameCategoryFn = createServerFn({ method: 'POST' })
  .validator((data: { id: string; name: string }) => data)
  .handler(async ({ data }) => (await updateCategory(data.id, { name: data.name })).status);

const deleteCategoryFn = createServerFn({ method: 'POST' })
  .validator((id: string) => id)
  .handler(async ({ data }) => (await deleteCategory(data)).status);

export const Route = createFileRoute('/admin/berita/')({
  loader: () => fetchNews(),
  head: () => ({ meta: [{ title: 'Berita · Pranata' }] }),
  component: News,
});

const root = getRouteApi('__root__');

const categoryErrors: Record<number, string> = {
  403: 'Hanya pengguna dengan izin menerbitkan konten yang dapat mengelola kategori.',
  404: 'Kategori ini sudah dihapus.',
  409: 'Nama kategori sudah dipakai, atau kategori masih dipakai berita.',
  422: 'Nama kategori wajib diisi, paling banyak 50 karakter.',
};

function News() {
  const { posts, forbidden, categories } = Route.useLoaderData();
  const village = root.useLoaderData();
  const router = useRouter();
  const navigate = useNavigate();
  const createPost = useServerFn(createPostFn);
  const addCategory = useServerFn(createCategoryFn);
  const renameCategory = useServerFn(renameCategoryFn);
  const removeCategory = useServerFn(deleteCategoryFn);
  const [writing, setWriting] = useState(false);
  const [dialogError, setDialogError] = useState('');
  const [message, setMessage] = useState<{ text: string; error: boolean } | null>(null);

  if (!posts) {
    return (
      <>
        <PageHead title="Berita" />
        <Notice error>
          {forbidden ? 'Anda tidak memiliki izin menulis konten.' : 'Gagal memuat berita.'}
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
    const result = await createPost({ data: title });
    if (!result.id) {
      setDialogError(
        result.status === 403
          ? 'Anda tidak memiliki izin menulis konten.'
          : 'Judul wajib diisi, paling banyak 150 karakter.',
      );
      return;
    }
    await navigate({ to: '/admin/berita/$id', params: { id: result.id } });
  }

  async function categoryResult(status: number, done: string) {
    if (status >= 300) {
      setMessage({ text: categoryErrors[status] ?? 'Gagal menyimpan kategori.', error: true });
      return false;
    }
    setMessage({ text: done, error: false });
    await router.invalidate();
    return true;
  }

  async function onAddCategory(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const form = event.currentTarget;
    const name = String(new FormData(form).get('name') ?? '').trim();
    if (await categoryResult(await addCategory({ data: name }), 'Kategori ditambahkan.')) {
      form.reset();
    }
  }

  async function onRename(category: Category) {
    const name = window.prompt('Nama kategori', category.name)?.trim();
    if (!name || name === category.name) return;
    await categoryResult(
      await renameCategory({ data: { id: category.id, name } }),
      'Kategori diubah.',
    );
  }

  async function onDeleteCategory(category: Category) {
    if (!window.confirm(`Hapus kategori ${category.name}?`)) return;
    await categoryResult(await removeCategory({ data: category.id }), 'Kategori dihapus.');
  }

  return (
    <>
      <PageHead title="Berita">
        Draf tersimpan otomatis dan belum tampil di situs; berita tampil setelah diterbitkan.
      </PageHead>
      {message && <Notice error={message.error}>{message.text}</Notice>}
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
        Tulis berita
      </Button>
      {posts.length === 0 ? (
        <p className="text-muted-foreground">Belum ada berita.</p>
      ) : (
        <DataTable>
          <TableHeader>
            <TableRow>
              <TableHead>Judul</TableHead>
              <TableHead>Kategori</TableHead>
              <TableHead>Status</TableHead>
              <TableHead>Diubah</TableHead>
              <TableHead>Penulis</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {posts.map((post) => (
              <TableRow key={post.id}>
                <TableCell className="max-w-96 truncate">
                  <Link
                    to="/admin/berita/$id"
                    params={{ id: post.id }}
                    className="font-medium underline-offset-4 hover:underline"
                  >
                    {post.title}
                  </Link>
                </TableCell>
                <TableCell>{post.category ?? '—'}</TableCell>
                <TableCell>
                  <Badge variant={post.published ? 'default' : 'secondary'}>
                    {post.published ? 'Terbit' : 'Draf'}
                  </Badge>
                </TableCell>
                <TableCell>{date.format(new Date(post.updatedAt))}</TableCell>
                <TableCell>{post.author}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </DataTable>
      )}
      <Card role="region" aria-labelledby="categories-title">
        <CardHeader>
          <h2 id="categories-title" className="text-base font-semibold">
            Kategori
          </h2>
        </CardHeader>
        <CardContent className="grid gap-3">
          {categories.length === 0 && <p className="text-muted-foreground">Belum ada kategori.</p>}
          <ul className="grid gap-1">
            {categories.map((category) => (
              <li key={category.id} className="flex flex-wrap items-center gap-2">
                <span className="flex-1">{category.name}</span>
                <Button type="button" variant="outline" onClick={() => onRename(category)}>
                  Ubah
                </Button>
                <Button type="button" variant="outline" onClick={() => onDeleteCategory(category)}>
                  Hapus
                </Button>
              </li>
            ))}
          </ul>
          <form onSubmit={onAddCategory} className="flex flex-wrap items-end gap-2">
            <div className="grid gap-2">
              <Label htmlFor="category-name">Kategori baru</Label>
              <Input id="category-name" name="name" required maxLength={50} />
            </div>
            <Button type="submit" variant="outline">
              Tambah kategori
            </Button>
          </form>
        </CardContent>
      </Card>
      <Dialog open={writing} onOpenChange={setWriting}>
        <DialogContent>
          <form onSubmit={onWrite} className="grid gap-4">
            <DialogHeader>
              <DialogTitle>Tulis berita</DialogTitle>
            </DialogHeader>
            <div className="grid gap-2">
              <Label htmlFor="new-post-title">Judul</Label>
              <Input id="new-post-title" name="title" required maxLength={150} />
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
