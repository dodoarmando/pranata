import { createFileRoute, getRouteApi, Link, useNavigate, useRouter } from '@tanstack/react-router';
import { createServerFn, useServerFn } from '@tanstack/react-start';
import type { JSONContent } from '@tiptap/core';
import { ArrowLeft } from 'lucide-react';
import { type ChangeEvent, useState } from 'react';
import {
  type Category,
  deleteNewsPost,
  getNewsPost,
  listCategories,
  type NewsPost,
  type NewsPostFields,
  publishNewsPost,
  unpublishNewsPost,
  updateNewsPost,
  uploadNewsPostCover,
} from '../../../api/generated';
import { useAutosave } from '../../../autosave';
import { Notice, PageHead } from '../../../components/admin';
import { RichTextEditor } from '../../../components/rich-text-editor';
import { Badge } from '../../../components/ui/badge';
import { Button } from '../../../components/ui/button';
import { Input } from '../../../components/ui/input';
import { Label } from '../../../components/ui/label';
import { NativeSelect, NativeSelectOption } from '../../../components/ui/native-select';
import { Textarea } from '../../../components/ui/textarea';
import { ianaZones } from '../../../village';

// Server-only; the post is null when it is gone or the User may not write content.
const fetchPost = createServerFn({ method: 'GET' })
  .validator((id: string) => id)
  .handler(async ({ data }) => {
    const [post, categories] = await Promise.all([getNewsPost(data), listCategories()]);
    return {
      // Typed as JSONContent so the router accepts the body as serializable.
      post: post.status === 200 ? { ...post.data, body: post.data.body as JSONContent } : null,
      status: post.status,
      categories: categories.status === 200 ? categories.data : [],
    };
  });

const saveFn = createServerFn({ method: 'POST' })
  .validator((data: { id: string; post: NewsPostFields }) => data)
  .handler(async ({ data }) => (await updateNewsPost(data.id, data.post)).status);

const publishFn = createServerFn({ method: 'POST' })
  .validator((data: { id: string; publish: boolean }) => data)
  .handler(
    async ({ data }) =>
      (await (data.publish ? publishNewsPost(data.id) : unpublishNewsPost(data.id))).status,
  );

const deleteFn = createServerFn({ method: 'POST' })
  .validator((id: string) => id)
  .handler(async ({ data }) => (await deleteNewsPost(data)).status);

const uploadCoverFn = createServerFn({ method: 'POST' })
  .validator((data: FormData) => {
    const cover = data.get('cover');
    if (!(cover instanceof File)) throw new Error('Expected a cover file');
    return cover;
  })
  .handler(async ({ data }) => {
    const response = await uploadNewsPostCover(data);
    return response.status === 201
      ? { status: response.status, id: response.data.id }
      : { status: response.status };
  });

export const Route = createFileRoute('/admin/berita/$id')({
  loader: ({ params }) => fetchPost({ data: params.id }),
  head: () => ({ meta: [{ title: 'Tulis berita · Pranata' }] }),
  component: Editor,
});

const root = getRouteApi('__root__');

const MAX_PHOTO_BYTES = 10 << 20;

const saveErrors: Record<number, string> = {
  403: 'Anda tidak memiliki izin mengubah berita ini; berita yang sudah terbit hanya dapat diubah oleh penerbit.',
  404: 'Berita ini sudah dihapus.',
  409: 'Slug sudah terkunci sejak pertama terbit, atau baru saja dipakai berita lain.',
  422: 'Periksa lagi isian: judul wajib diisi (paling banyak 150 karakter), ringkasan paling banyak 250 karakter, dan berita yang terbit wajib punya kategori, ringkasan, foto sampul, dan teks alternatif.',
};

const publishErrors: Record<number, string> = {
  403: 'Hanya pengguna dengan izin menerbitkan konten yang dapat menerbitkan, membatalkan terbit, atau menghapus berita.',
  404: 'Berita ini sudah dihapus.',
  409: 'Status berita sudah berubah; muat ulang halaman.',
  422: 'Sebelum terbit, lengkapi kategori, ringkasan, foto sampul, dan teks alternatifnya.',
};

const coverErrors: Record<number, string> = {
  403: 'Anda tidak memiliki izin menulis konten.',
  413: 'Foto paling besar 10 MB.',
  415: 'Foto harus berupa berkas JPEG, PNG, atau WebP.',
  422: 'Gambar rusak atau lebih dari 40 megapiksel.',
};

function Editor() {
  const { post, status, categories } = Route.useLoaderData();
  if (!post) {
    return (
      <>
        <PageHead title="Berita" />
        <Notice error>
          {status === 403
            ? 'Anda tidak memiliki izin menulis konten.'
            : status === 404
              ? 'Berita ini sudah dihapus.'
              : 'Gagal memuat berita.'}
        </Notice>
      </>
    );
  }
  // Keyed, so the form starts over from the server's copy when another post opens.
  return <Form key={post.id} post={post} categories={categories} />;
}

function Form({
  post,
  categories,
}: {
  post: NewsPost & { body: JSONContent };
  categories: Category[];
}) {
  const village = root.useLoaderData();
  const router = useRouter();
  const navigate = useNavigate();
  const save = useServerFn(saveFn);
  const setPublished = useServerFn(publishFn);
  const remove = useServerFn(deleteFn);
  const uploadCover = useServerFn(uploadCoverFn);
  const [uploading, setUploading] = useState(false);
  const [message, setMessage] = useState<{ text: string; error: boolean } | null>(null);
  const { fields, change, dirty, saving, savedAt, flush, discard } = useAutosave<NewsPostFields>(
    () => ({
      title: post.title,
      slug: post.slug ?? null,
      category: post.category ?? null,
      summary: post.summary ?? null,
      cover: post.cover ?? null,
      coverCaption: post.coverCaption ?? null,
      coverAlt: post.coverAlt ?? null,
      body: post.body,
    }),
    post.published,
    (saved) => save({ data: { id: post.id, post: saved } }),
    (result) =>
      setMessage(
        result >= 300 ? { text: saveErrors[result] ?? 'Gagal menyimpan.', error: true } : null,
      ),
  );
  const locked = Boolean(post.firstPublishedAt);
  const time = new Intl.DateTimeFormat('id-ID', {
    timeStyle: 'short',
    timeZone: ianaZones[village?.timeZone ?? 'WIB'],
  });

  async function onCover(event: ChangeEvent<HTMLInputElement>) {
    const file = event.currentTarget.files?.[0];
    event.currentTarget.value = '';
    if (!file) return;
    if (file.size > MAX_PHOTO_BYTES) {
      setMessage({ text: coverErrors[413] ?? '', error: true });
      return;
    }
    setUploading(true);
    const data = new FormData();
    data.set('cover', file);
    const result = await uploadCover({ data }).finally(() => setUploading(false));
    if (result.id) {
      change({ cover: result.id });
      setMessage(null);
    } else {
      setMessage({ text: coverErrors[result.status] ?? 'Gagal mengunggah foto.', error: true });
    }
  }

  function onCaption(caption: string) {
    // The alt text follows the caption until the writer makes it their own.
    const followsCaption = !fields.coverAlt || fields.coverAlt === fields.coverCaption;
    change(
      followsCaption ? { coverCaption: caption, coverAlt: caption } : { coverCaption: caption },
    );
  }

  async function onPublish(publish: boolean) {
    if (dirty && !post.published && !(await flush())) return;
    const result = await setPublished({ data: { id: post.id, publish } });
    if (result >= 300) {
      setMessage({ text: publishErrors[result] ?? 'Gagal mengubah status.', error: true });
      return;
    }
    setMessage({
      text: publish ? 'Berita terbit di situs.' : 'Berita kembali menjadi draf.',
      error: false,
    });
    await router.invalidate();
  }

  async function onDelete() {
    if (!window.confirm(`Hapus draf "${post.title}" untuk selamanya?`)) return;
    const result = await remove({ data: post.id });
    if (result >= 300) {
      setMessage({ text: publishErrors[result] ?? 'Gagal menghapus.', error: true });
      return;
    }
    discard();
    await navigate({ to: '/admin/berita' });
  }

  const status = post.published
    ? dirty
      ? 'Ada perubahan yang belum disimpan.'
      : 'Terbit di situs.'
    : saving
      ? 'Menyimpan…'
      : dirty
        ? 'Perubahan akan tersimpan otomatis.'
        : savedAt
          ? `Tersimpan otomatis pukul ${time.format(savedAt)}.`
          : 'Draf, belum tampil di situs.';

  return (
    <>
      <Link
        to="/admin/berita"
        className="flex w-fit items-center gap-1 text-muted-foreground hover:text-foreground"
      >
        <ArrowLeft className="size-4" aria-hidden="true" />
        Semua berita
      </Link>
      <div className="flex flex-wrap items-center gap-3">
        <h1 className="text-xl font-semibold">{post.published ? 'Ubah berita' : 'Tulis berita'}</h1>
        <Badge variant={post.published ? 'default' : 'secondary'}>
          {post.published ? 'Terbit' : 'Draf'}
        </Badge>
        <p role="status" className="text-muted-foreground">
          {status}
        </p>
      </div>
      {message && <Notice error={message.error}>{message.text}</Notice>}
      <div className="flex flex-wrap gap-2">
        {post.published ? (
          <>
            <Button type="button" size="lg" className="px-4" disabled={saving} onClick={flush}>
              Simpan
            </Button>
            <Button
              type="button"
              variant="outline"
              size="lg"
              className="px-4"
              onClick={() => onPublish(false)}
            >
              Batalkan terbit
            </Button>
            <Button
              variant="outline"
              size="lg"
              className="px-4"
              render={<a href={`/berita/${post.slug}`} target="_blank" rel="noopener" />}
            >
              Lihat di situs
            </Button>
          </>
        ) : (
          <>
            <Button
              type="button"
              size="lg"
              className="px-4"
              disabled={saving || uploading}
              onClick={() => onPublish(true)}
            >
              Terbitkan
            </Button>
            <Button type="button" variant="outline" size="lg" className="px-4" onClick={onDelete}>
              Hapus draf
            </Button>
          </>
        )}
      </div>
      <div className="grid max-w-4xl gap-5">
        <div className="grid gap-2">
          <Label htmlFor="post-title">Judul</Label>
          <Input
            id="post-title"
            required
            maxLength={150}
            value={fields.title}
            onChange={(event) => change({ title: event.currentTarget.value })}
          />
        </div>
        <div className="grid gap-2">
          <Label htmlFor="post-slug">Slug</Label>
          <Input
            id="post-slug"
            maxLength={80}
            value={fields.slug ?? ''}
            disabled={locked}
            aria-describedby="post-slug-hint"
            onChange={(event) => change({ slug: event.currentTarget.value })}
          />
          <p id="post-slug-hint" className="text-sm text-muted-foreground">
            {locked
              ? `Terkunci sejak pertama terbit, agar tautan yang sudah dibagikan tidak rusak: /berita/${post.slug}`
              : `Alamat berita: /berita/${post.slug}. Kosongkan agar dibuat dari judul.`}
          </p>
        </div>
        <div className="grid gap-2">
          <Label htmlFor="post-category">Kategori</Label>
          <NativeSelect
            id="post-category"
            value={fields.category ?? ''}
            onChange={(event) => change({ category: event.currentTarget.value || null })}
          >
            <NativeSelectOption value="">Pilih kategori</NativeSelectOption>
            {categories.map((category) => (
              <NativeSelectOption key={category.id} value={category.id}>
                {category.name}
              </NativeSelectOption>
            ))}
          </NativeSelect>
        </div>
        <div className="grid gap-2">
          <Label htmlFor="post-summary">Ringkasan</Label>
          <Textarea
            id="post-summary"
            maxLength={250}
            rows={3}
            value={fields.summary ?? ''}
            aria-describedby="post-summary-hint"
            onChange={(event) => change({ summary: event.currentTarget.value })}
          />
          <p id="post-summary-hint" className="text-sm text-muted-foreground">
            Sekitar 200 karakter: pembuka berita dan deskripsi di mesin pencari.{' '}
            {(fields.summary ?? '').length}/250
          </p>
        </div>
        <fieldset className="grid gap-3">
          <legend className="mb-2 text-sm font-medium">Foto sampul</legend>
          {fields.cover ? (
            <img
              src={`/media/${fields.cover}/960`}
              alt=""
              width={480}
              height={270}
              className="aspect-video w-full max-w-120 rounded-lg border object-cover"
            />
          ) : (
            <span className="grid aspect-video w-full max-w-120 place-items-center rounded-lg border text-muted-foreground">
              Belum ada foto
            </span>
          )}
          <div className="grid gap-2">
            <Label htmlFor="post-cover">
              Unggah foto (JPEG, PNG, atau WebP, paling besar 10 MB)
            </Label>
            <Input
              id="post-cover"
              type="file"
              accept="image/jpeg,image/png,image/webp"
              onChange={onCover}
              disabled={uploading}
            />
          </div>
          <div className="grid gap-2">
            <Label htmlFor="post-caption">Keterangan foto</Label>
            <Input
              id="post-caption"
              maxLength={300}
              value={fields.coverCaption ?? ''}
              onChange={(event) => onCaption(event.currentTarget.value)}
            />
          </div>
          <div className="grid gap-2">
            <Label htmlFor="post-alt">Teks alternatif</Label>
            <Input
              id="post-alt"
              maxLength={300}
              value={fields.coverAlt ?? ''}
              aria-describedby="post-alt-hint"
              onChange={(event) => change({ coverAlt: event.currentTarget.value })}
            />
            <p id="post-alt-hint" className="text-sm text-muted-foreground">
              Gambaran foto untuk pembaca tunanetra; diisi dari keterangan foto, ubah bila perlu.
            </p>
          </div>
        </fieldset>
        <div className="grid gap-2">
          <span id="post-body-label" className="text-sm font-medium">
            Isi berita
          </span>
          <RichTextEditor
            content={post.body ?? { type: 'doc', content: [] }}
            labelledBy="post-body-label"
            onChange={(body) => change({ body })}
          />
        </div>
      </div>
    </>
  );
}
