import { createFileRoute, getRouteApi, Link, useNavigate, useRouter } from '@tanstack/react-router';
import { createServerFn, useServerFn } from '@tanstack/react-start';
import { ArrowDown, ArrowLeft, ArrowUp, Trash2 } from 'lucide-react';
import { type ChangeEvent, useState } from 'react';
import {
  type Album,
  type AlbumFields,
  type AlbumPhoto,
  deleteAlbum,
  getAlbum,
  publishAlbum,
  unpublishAlbum,
  updateAlbum,
  uploadAlbumPhoto,
} from '../../../api/generated';
import { useAutosave } from '../../../autosave';
import { Notice, PageHead } from '../../../components/admin';
import { Badge } from '../../../components/ui/badge';
import { Button } from '../../../components/ui/button';
import { Input } from '../../../components/ui/input';
import { Label } from '../../../components/ui/label';
import { Textarea } from '../../../components/ui/textarea';
import { ianaZones } from '../../../village';

// Server-only; the album is null when it is gone or the User may not write content.
const fetchAlbum = createServerFn({ method: 'GET' })
  .validator((id: string) => id)
  .handler(async ({ data }) => {
    const album = await getAlbum(data);
    return { album: album.status === 200 ? album.data : null, status: album.status };
  });

const saveFn = createServerFn({ method: 'POST' })
  .validator((data: { id: string; album: AlbumFields }) => data)
  .handler(async ({ data }) => (await updateAlbum(data.id, data.album)).status);

const publishFn = createServerFn({ method: 'POST' })
  .validator((data: { id: string; publish: boolean }) => data)
  .handler(
    async ({ data }) =>
      (await (data.publish ? publishAlbum(data.id) : unpublishAlbum(data.id))).status,
  );

const deleteFn = createServerFn({ method: 'POST' })
  .validator((id: string) => id)
  .handler(async ({ data }) => (await deleteAlbum(data)).status);

const uploadPhotoFn = createServerFn({ method: 'POST' })
  .validator((data: FormData) => {
    const photo = data.get('photo');
    if (!(photo instanceof File)) throw new Error('Expected a photo file');
    return photo;
  })
  .handler(async ({ data }) => {
    const response = await uploadAlbumPhoto(data);
    return response.status === 201
      ? { status: response.status, id: response.data.id }
      : { status: response.status };
  });

export const Route = createFileRoute('/admin/galeri/$id')({
  loader: ({ params }) => fetchAlbum({ data: params.id }),
  head: () => ({ meta: [{ title: 'Album · Pranata' }] }),
  component: Editor,
});

const root = getRouteApi('__root__');

const MAX_PHOTO_BYTES = 10 << 20;

const saveErrors: Record<number, string> = {
  403: 'Anda tidak memiliki izin mengubah album ini; album yang sudah terbit hanya dapat diubah oleh penerbit.',
  404: 'Album ini sudah dihapus.',
  409: 'Slug sudah terkunci sejak pertama terbit, atau baru saja dipakai album lain.',
  422: 'Periksa lagi isian: judul wajib diisi (paling banyak 150 karakter), tanggal selesai tidak boleh sebelum tanggal mulai, dan album yang terbit wajib punya foto dengan teks alternatif.',
};

const publishErrors: Record<number, string> = {
  403: 'Hanya pengguna dengan izin menerbitkan konten yang dapat menerbitkan, membatalkan terbit, atau menghapus album.',
  404: 'Album ini sudah dihapus.',
  409: 'Status album sudah berubah; muat ulang.',
  422: 'Sebelum terbit, isi tanggal mulai kegiatan dan tambahkan minimal satu foto; setiap foto wajib punya teks alternatif.',
};

const photoErrors: Record<number, string> = {
  403: 'Anda tidak memiliki izin menulis konten.',
  413: 'Setiap foto paling besar 10 MB.',
  415: 'Foto harus berupa berkas JPEG, PNG, atau WebP.',
  422: 'Gambar rusak atau lebih dari 40 megapiksel.',
};

function Editor() {
  const { album, status } = Route.useLoaderData();
  if (!album) {
    return (
      <>
        <PageHead title="Album" />
        <Notice error>
          {status === 403
            ? 'Anda tidak memiliki izin menulis konten.'
            : status === 404
              ? 'Album ini sudah dihapus.'
              : 'Gagal memuat album.'}
        </Notice>
      </>
    );
  }
  // Keyed, so the form starts over from the server's copy when another album opens.
  return <Form key={album.id} album={album} />;
}

function Form({ album }: { album: Album }) {
  const village = root.useLoaderData();
  const router = useRouter();
  const navigate = useNavigate();
  const save = useServerFn(saveFn);
  const setPublished = useServerFn(publishFn);
  const remove = useServerFn(deleteFn);
  const uploadPhoto = useServerFn(uploadPhotoFn);
  const [uploading, setUploading] = useState(false);
  const [message, setMessage] = useState<{ text: string; error: boolean } | null>(null);
  const { fields, change, dirty, saving, savedAt, flush, discard } = useAutosave<AlbumFields>(
    () => ({
      title: album.title,
      slug: album.slug ?? null,
      description: album.description ?? null,
      occasionStart: album.occasionStart ?? null,
      occasionEnd: album.occasionEnd ?? null,
      photos: album.photos ?? [],
      cover: album.cover ?? null,
    }),
    album.published,
    (saved) => save({ data: { id: album.id, album: saved } }),
    (result) =>
      setMessage(
        result >= 300 ? { text: saveErrors[result] ?? 'Gagal menyimpan.', error: true } : null,
      ),
  );
  const photos = fields.photos ?? [];
  const cover = fields.cover ?? photos[0]?.file;
  const locked = Boolean(album.firstPublishedAt);
  const time = new Intl.DateTimeFormat('id-ID', {
    timeStyle: 'short',
    timeZone: ianaZones[village?.timeZone ?? 'WIB'],
  });

  async function onPhotos(event: ChangeEvent<HTMLInputElement>) {
    const files = [...(event.currentTarget.files ?? [])];
    event.currentTarget.value = '';
    if (files.some((file) => file.size > MAX_PHOTO_BYTES)) {
      setMessage({ text: photoErrors[413] ?? '', error: true });
      return;
    }
    setUploading(true);
    const added: AlbumPhoto[] = [];
    let failed: number | undefined;
    try {
      // One at a time, in the order picked, so the album keeps that order.
      for (const file of files) {
        const data = new FormData();
        data.set('photo', file);
        const result = await uploadPhoto({ data });
        if (!result.id) {
          failed = result.status;
          break;
        }
        added.push({ file: result.id, caption: null, alt: null });
      }
    } finally {
      setUploading(false);
    }
    if (added.length) change({ photos: [...photos, ...added] });
    setMessage(
      failed === undefined
        ? null
        : { text: photoErrors[failed] ?? 'Gagal mengunggah foto.', error: true },
    );
  }

  function changePhoto(index: number, patch: Partial<AlbumPhoto>) {
    change({ photos: photos.map((photo, at) => (at === index ? { ...photo, ...patch } : photo)) });
  }

  function onCaption(index: number, caption: string) {
    const photo = photos[index];
    // The alt text follows the caption until the writer makes it their own.
    const followsCaption = !photo?.alt || photo.alt === photo.caption;
    changePhoto(index, followsCaption ? { caption, alt: caption } : { caption });
  }

  function move(index: number, by: number) {
    const moved = [...photos];
    const [photo] = moved.splice(index, 1);
    if (photo) moved.splice(index + by, 0, photo);
    change({ photos: moved });
  }

  function removePhoto(index: number) {
    const file = photos[index]?.file;
    change({
      photos: photos.filter((_, at) => at !== index),
      ...(fields.cover === file && { cover: null }),
    });
  }

  async function onPublish(publish: boolean) {
    if (dirty && !album.published && !(await flush())) return;
    const result = await setPublished({ data: { id: album.id, publish } });
    if (result >= 300) {
      setMessage({ text: publishErrors[result] ?? 'Gagal mengubah status.', error: true });
      return;
    }
    setMessage({
      text: publish ? 'Album terbit di situs.' : 'Album kembali menjadi draf.',
      error: false,
    });
    await router.invalidate();
  }

  async function onDelete() {
    if (!window.confirm(`Hapus draf "${album.title}" untuk selamanya?`)) return;
    const result = await remove({ data: album.id });
    if (result >= 300) {
      setMessage({ text: publishErrors[result] ?? 'Gagal menghapus.', error: true });
      return;
    }
    discard();
    await navigate({ to: '/admin/galeri' });
  }

  const status = album.published
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
        to="/admin/galeri"
        className="flex w-fit items-center gap-1 text-muted-foreground hover:text-foreground"
      >
        <ArrowLeft className="size-4" aria-hidden="true" />
        Semua album
      </Link>
      <div className="flex flex-wrap items-center gap-3">
        <h1 className="text-xl font-semibold">{album.published ? 'Ubah album' : 'Buat album'}</h1>
        <Badge variant={album.published ? 'default' : 'secondary'}>
          {album.published ? 'Terbit' : 'Draf'}
        </Badge>
        <p role="status" className="text-muted-foreground">
          {status}
        </p>
      </div>
      {message && <Notice error={message.error}>{message.text}</Notice>}
      <div className="flex flex-wrap gap-2">
        {album.published ? (
          <>
            <Button
              type="button"
              size="lg"
              className="px-4"
              disabled={saving || uploading}
              onClick={flush}
            >
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
              render={<a href={`/galeri/${album.slug}`} target="_blank" rel="noopener" />}
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
          <Label htmlFor="album-title">Judul</Label>
          <Input
            id="album-title"
            required
            maxLength={150}
            value={fields.title}
            onChange={(event) => change({ title: event.currentTarget.value })}
          />
        </div>
        <div className="grid gap-2">
          <Label htmlFor="album-slug">Slug</Label>
          <Input
            id="album-slug"
            maxLength={80}
            value={fields.slug ?? ''}
            disabled={locked}
            aria-describedby="album-slug-hint"
            onChange={(event) => change({ slug: event.currentTarget.value })}
          />
          <p id="album-slug-hint" className="text-sm text-muted-foreground">
            {locked
              ? `Slug terkunci sejak pertama terbit, agar tautan yang sudah dibagikan tidak rusak: /galeri/${album.slug}`
              : `Alamat album: /galeri/${album.slug}. Kosongkan agar dibuat dari judul.`}
          </p>
        </div>
        <div className="grid gap-2">
          <Label htmlFor="album-description">Deskripsi (opsional)</Label>
          <Textarea
            id="album-description"
            maxLength={500}
            rows={3}
            value={fields.description ?? ''}
            aria-describedby="album-description-hint"
            onChange={(event) => change({ description: event.currentTarget.value })}
          />
          <p id="album-description-hint" className="text-sm text-muted-foreground">
            {(fields.description ?? '').length}/500
          </p>
        </div>
        <fieldset className="grid gap-2">
          <legend className="mb-2 text-sm font-medium">Tanggal kegiatan</legend>
          <div className="flex flex-wrap gap-4">
            <div className="grid gap-2">
              <Label htmlFor="album-start">Mulai</Label>
              <Input
                id="album-start"
                type="date"
                value={fields.occasionStart ?? ''}
                onChange={(event) => change({ occasionStart: event.currentTarget.value || null })}
              />
            </div>
            <div className="grid gap-2">
              <Label htmlFor="album-end">Selesai (opsional)</Label>
              <Input
                id="album-end"
                type="date"
                min={fields.occasionStart ?? undefined}
                value={fields.occasionEnd ?? ''}
                onChange={(event) => change({ occasionEnd: event.currentTarget.value || null })}
              />
            </div>
          </div>
        </fieldset>
        <fieldset className="grid gap-4" disabled={uploading}>
          <legend className="mb-2 text-sm font-medium">Foto ({photos.length})</legend>
          {photos.length === 0 && (
            <p className="text-muted-foreground">
              Belum ada foto; album tanpa foto tidak dapat terbit.
            </p>
          )}
          <ol className="grid gap-4">
            {photos.map((photo, index) => (
              <li
                key={photo.file}
                className="grid gap-4 rounded-lg border p-4 sm:grid-cols-[12rem_1fr]"
              >
                <img
                  src={`/media/${photo.file}/480`}
                  alt=""
                  width={192}
                  height={128}
                  className="aspect-3/2 w-full rounded-md border object-cover"
                />
                <div className="grid gap-3">
                  <p className="text-sm font-medium">Foto {index + 1}</p>
                  <div className="grid gap-2">
                    <Label htmlFor={`photo-${photo.file}-caption`}>Keterangan (opsional)</Label>
                    <Input
                      id={`photo-${photo.file}-caption`}
                      maxLength={300}
                      value={photo.caption ?? ''}
                      onChange={(event) => onCaption(index, event.currentTarget.value)}
                    />
                  </div>
                  <div className="grid gap-2">
                    <Label htmlFor={`photo-${photo.file}-alt`}>Teks alternatif</Label>
                    <Input
                      id={`photo-${photo.file}-alt`}
                      maxLength={300}
                      required
                      value={photo.alt ?? ''}
                      onChange={(event) => changePhoto(index, { alt: event.currentTarget.value })}
                    />
                  </div>
                  <div className="flex flex-wrap items-center gap-2">
                    <label className="mr-2 flex items-center gap-2 text-sm">
                      <input
                        type="radio"
                        name="album-cover"
                        checked={cover === photo.file}
                        onChange={() => change({ cover: photo.file })}
                      />
                      Sampul album
                    </label>
                    <Button
                      type="button"
                      variant="outline"
                      size="icon"
                      aria-label={`Naikkan foto ${index + 1}`}
                      disabled={index === 0}
                      onClick={() => move(index, -1)}
                    >
                      <ArrowUp aria-hidden="true" />
                    </Button>
                    <Button
                      type="button"
                      variant="outline"
                      size="icon"
                      aria-label={`Turunkan foto ${index + 1}`}
                      disabled={index === photos.length - 1}
                      onClick={() => move(index, 1)}
                    >
                      <ArrowDown aria-hidden="true" />
                    </Button>
                    <Button
                      type="button"
                      variant="outline"
                      size="icon"
                      aria-label={`Hapus foto ${index + 1}`}
                      onClick={() => removePhoto(index)}
                    >
                      <Trash2 aria-hidden="true" />
                    </Button>
                  </div>
                </div>
              </li>
            ))}
          </ol>
          <div className="grid gap-2">
            <Label htmlFor="album-photos">
              Tambah foto (JPEG, PNG, atau WebP, masing-masing paling besar 10 MB)
            </Label>
            <Input
              id="album-photos"
              type="file"
              multiple
              accept="image/jpeg,image/png,image/webp"
              aria-describedby="album-photos-hint"
              onChange={onPhotos}
            />
            <p id="album-photos-hint" className="text-sm text-muted-foreground">
              {uploading
                ? 'Mengunggah foto…'
                : 'Teks alternatif menggambarkan foto untuk pembaca tunanetra; diisi dari keterangan, ubah bila perlu.'}
            </p>
          </div>
        </fieldset>
      </div>
    </>
  );
}
