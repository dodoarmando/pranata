import { createFileRoute, getRouteApi, Link, useNavigate, useRouter } from '@tanstack/react-router';
import { createServerFn, useServerFn } from '@tanstack/react-start';
import type { JSONContent } from '@tiptap/core';
import { ArrowLeft } from 'lucide-react';
import { type ChangeEvent, useState } from 'react';
import {
  deletePage,
  getPage,
  type Page,
  type PageFields,
  type PageMenu,
  publishPage,
  unpublishPage,
  updatePage,
  uploadPageCover,
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
import { pageMenus, pagePath } from '../../../page';
import { ianaZones } from '../../../village';

// Server-only; the page is null when it is gone or the User may not write content.
const fetchPage = createServerFn({ method: 'GET' })
  .validator((id: string) => id)
  .handler(async ({ data }) => {
    const page = await getPage(data);
    return {
      // Typed as JSONContent so the router accepts the body as serializable.
      page: page.status === 200 ? { ...page.data, body: page.data.body as JSONContent } : null,
      status: page.status,
    };
  });

const saveFn = createServerFn({ method: 'POST' })
  .validator((data: { id: string; page: PageFields }) => data)
  .handler(async ({ data }) => (await updatePage(data.id, data.page)).status);

const publishFn = createServerFn({ method: 'POST' })
  .validator((data: { id: string; publish: boolean }) => data)
  .handler(
    async ({ data }) =>
      (await (data.publish ? publishPage(data.id) : unpublishPage(data.id))).status,
  );

const deleteFn = createServerFn({ method: 'POST' })
  .validator((id: string) => id)
  .handler(async ({ data }) => (await deletePage(data)).status);

const uploadCoverFn = createServerFn({ method: 'POST' })
  .validator((data: FormData) => {
    const cover = data.get('cover');
    if (!(cover instanceof File)) throw new Error('Expected a cover file');
    return cover;
  })
  .handler(async ({ data }) => {
    const response = await uploadPageCover(data);
    return response.status === 201
      ? { status: response.status, id: response.data.id }
      : { status: response.status };
  });

export const Route = createFileRoute('/admin/halaman/$id')({
  loader: ({ params }) => fetchPage({ data: params.id }),
  head: () => ({ meta: [{ title: 'Tulis halaman · Pranata' }] }),
  component: Editor,
});

const root = getRouteApi('__root__');

const MAX_PHOTO_BYTES = 10 << 20;

const saveErrors: Record<number, string> = {
  403: 'Anda tidak memiliki izin mengubah halaman ini; halaman yang sudah terbit hanya dapat diubah oleh penerbit.',
  404: 'Halaman ini sudah dihapus.',
  409: 'Slug dan menu sudah terkunci sejak pertama terbit, atau slug baru saja dipakai halaman lain.',
  422: 'Periksa lagi isian: judul wajib diisi (paling banyak 150 karakter), ringkasan paling banyak 250 karakter, dan halaman yang terbit wajib punya ringkasan serta teks alternatif bila ada foto sampul.',
};

const publishErrors: Record<number, string> = {
  403: 'Hanya pengguna dengan izin menerbitkan konten yang dapat menerbitkan, membatalkan terbit, atau menghapus halaman.',
  404: 'Halaman ini sudah dihapus.',
  409: 'Status halaman sudah berubah; muat ulang.',
  422: 'Sebelum terbit, lengkapi ringkasan, dan teks alternatif bila ada foto sampul.',
};

const coverErrors: Record<number, string> = {
  403: 'Anda tidak memiliki izin menulis konten.',
  413: 'Foto paling besar 10 MB.',
  415: 'Foto harus berupa berkas JPEG, PNG, atau WebP.',
  422: 'Gambar rusak atau lebih dari 40 megapiksel.',
};

function Editor() {
  const { page, status } = Route.useLoaderData();
  if (!page) {
    return (
      <>
        <PageHead title="Halaman" />
        <Notice error>
          {status === 403
            ? 'Anda tidak memiliki izin menulis konten.'
            : status === 404
              ? 'Halaman ini sudah dihapus.'
              : 'Gagal memuat halaman.'}
        </Notice>
      </>
    );
  }
  // Keyed, so the form starts over from the server's copy when another page opens.
  return <Form key={page.id} page={page} />;
}

function Form({ page }: { page: Page & { body: JSONContent } }) {
  const village = root.useLoaderData();
  const router = useRouter();
  const navigate = useNavigate();
  const save = useServerFn(saveFn);
  const setPublished = useServerFn(publishFn);
  const remove = useServerFn(deleteFn);
  const uploadCover = useServerFn(uploadCoverFn);
  const [uploading, setUploading] = useState(false);
  const [message, setMessage] = useState<{ text: string; error: boolean } | null>(null);
  const { fields, change, dirty, saving, savedAt, flush, discard } = useAutosave<PageFields>(
    () => ({
      title: page.title,
      slug: page.slug ?? null,
      menu: page.menu,
      order: page.order ?? null,
      summary: page.summary ?? null,
      cover: page.cover ?? null,
      coverCaption: page.coverCaption ?? null,
      coverAlt: page.coverAlt ?? null,
      body: page.body,
    }),
    page.published,
    (saved) => save({ data: { id: page.id, page: saved } }),
    (result) =>
      setMessage(
        result >= 300 ? { text: saveErrors[result] ?? 'Gagal menyimpan.', error: true } : null,
      ),
  );
  const locked = Boolean(page.firstPublishedAt);
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
    if (dirty && !page.published && !(await flush())) return;
    const result = await setPublished({ data: { id: page.id, publish } });
    if (result >= 300) {
      setMessage({ text: publishErrors[result] ?? 'Gagal mengubah status.', error: true });
      return;
    }
    setMessage({
      text: publish ? 'Halaman terbit di situs.' : 'Halaman kembali menjadi draf.',
      error: false,
    });
    await router.invalidate();
  }

  async function onDelete() {
    if (!window.confirm(`Hapus draf "${page.title}" untuk selamanya?`)) return;
    const result = await remove({ data: page.id });
    if (result >= 300) {
      setMessage({ text: publishErrors[result] ?? 'Gagal menghapus.', error: true });
      return;
    }
    discard();
    await navigate({ to: '/admin/halaman' });
  }

  const status = page.published
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
        to="/admin/halaman"
        className="flex w-fit items-center gap-1 text-muted-foreground hover:text-foreground"
      >
        <ArrowLeft className="size-4" aria-hidden="true" />
        Semua halaman
      </Link>
      <div className="flex flex-wrap items-center gap-3">
        <h1 className="text-xl font-semibold">
          {page.published ? 'Ubah halaman' : 'Tulis halaman'}
        </h1>
        <Badge variant={page.published ? 'default' : 'secondary'}>
          {page.published ? 'Terbit' : 'Draf'}
        </Badge>
        <p role="status" className="text-muted-foreground">
          {status}
        </p>
      </div>
      {message && <Notice error={message.error}>{message.text}</Notice>}
      <div className="flex flex-wrap gap-2">
        {page.published ? (
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
              render={
                <a href={pagePath(page.menu, page.slug ?? '')} target="_blank" rel="noopener" />
              }
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
          <Label htmlFor="page-title">Judul</Label>
          <Input
            id="page-title"
            required
            maxLength={150}
            value={fields.title}
            onChange={(event) => change({ title: event.currentTarget.value })}
          />
        </div>
        <div className="grid gap-2">
          <Label htmlFor="page-slug">Slug</Label>
          <Input
            id="page-slug"
            maxLength={80}
            value={fields.slug ?? ''}
            disabled={locked}
            aria-describedby="page-slug-hint"
            onChange={(event) => change({ slug: event.currentTarget.value })}
          />
          <p id="page-slug-hint" className="text-sm text-muted-foreground">
            {locked
              ? `Slug dan menu terkunci sejak pertama terbit, agar tautan yang sudah dibagikan tidak rusak: ${pagePath(page.menu, page.slug ?? '')}`
              : `Alamat halaman: ${pagePath(page.menu, page.slug ?? '')}. Kosongkan agar dibuat dari judul.`}
          </p>
        </div>
        <div className="grid gap-2">
          <Label htmlFor="page-menu">Menu</Label>
          <NativeSelect
            id="page-menu"
            value={fields.menu}
            disabled={locked}
            onChange={(event) => change({ menu: event.currentTarget.value as PageMenu })}
          >
            {Object.entries(pageMenus).map(([menu, { label }]) => (
              <NativeSelectOption key={menu} value={menu}>
                {label}
              </NativeSelectOption>
            ))}
          </NativeSelect>
        </div>
        <div className="grid gap-2">
          <Label htmlFor="page-order">Nomor urut</Label>
          <Input
            id="page-order"
            type="number"
            step={1}
            className="w-32"
            value={fields.order ?? ''}
            aria-describedby="page-order-hint"
            onChange={(event) => {
              const order = event.currentTarget.valueAsNumber;
              change({ order: Number.isInteger(order) ? order : null });
            }}
          />
          <p id="page-order-hint" className="text-sm text-muted-foreground">
            Halaman dengan nomor terkecil tampil paling atas di menunya.
          </p>
        </div>
        <div className="grid gap-2">
          <Label htmlFor="page-summary">Ringkasan</Label>
          <Textarea
            id="page-summary"
            maxLength={250}
            rows={3}
            value={fields.summary ?? ''}
            aria-describedby="page-summary-hint"
            onChange={(event) => change({ summary: event.currentTarget.value })}
          />
          <p id="page-summary-hint" className="text-sm text-muted-foreground">
            Deskripsi halaman di mesin pencari dan saat tautan dibagikan.{' '}
            {(fields.summary ?? '').length}/250
          </p>
        </div>
        <fieldset className="grid gap-3">
          <legend className="mb-2 text-sm font-medium">Foto sampul (opsional)</legend>
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
          {fields.cover && (
            <Button
              type="button"
              variant="outline"
              className="w-fit"
              onClick={() => change({ cover: null })}
            >
              Hapus foto
            </Button>
          )}
          <div className="grid gap-2">
            <Label htmlFor="page-cover">
              Unggah foto (JPEG, PNG, atau WebP, paling besar 10 MB)
            </Label>
            <Input
              id="page-cover"
              type="file"
              accept="image/jpeg,image/png,image/webp"
              onChange={onCover}
              disabled={uploading}
            />
          </div>
          <div className="grid gap-2">
            <Label htmlFor="page-caption">Keterangan foto</Label>
            <Input
              id="page-caption"
              maxLength={300}
              value={fields.coverCaption ?? ''}
              onChange={(event) => onCaption(event.currentTarget.value)}
            />
          </div>
          <div className="grid gap-2">
            <Label htmlFor="page-alt">Teks alternatif</Label>
            <Input
              id="page-alt"
              maxLength={300}
              value={fields.coverAlt ?? ''}
              aria-describedby="page-alt-hint"
              onChange={(event) => change({ coverAlt: event.currentTarget.value })}
            />
            <p id="page-alt-hint" className="text-sm text-muted-foreground">
              Gambaran foto untuk pembaca tunanetra; diisi dari keterangan foto, ubah bila perlu.
            </p>
          </div>
        </fieldset>
        <div className="grid gap-2">
          <span id="page-body-label" className="text-sm font-medium">
            Isi halaman
          </span>
          <RichTextEditor
            content={page.body ?? { type: 'doc', content: [] }}
            labelledBy="page-body-label"
            onChange={(body) => change({ body })}
          />
        </div>
      </div>
    </>
  );
}
