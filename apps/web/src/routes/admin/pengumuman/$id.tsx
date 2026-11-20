import { createFileRoute, getRouteApi, Link, useNavigate, useRouter } from '@tanstack/react-router';
import { createServerFn, useServerFn } from '@tanstack/react-start';
import type { JSONContent } from '@tiptap/core';
import { ArrowLeft } from 'lucide-react';
import { useState } from 'react';
import {
  type Announcement,
  type AnnouncementFields,
  deleteAnnouncement,
  getAnnouncement,
  publishAnnouncement,
  unpublishAnnouncement,
  updateAnnouncement,
} from '../../../api/generated';
import { useAutosave } from '../../../autosave';
import { Notice, PageHead } from '../../../components/admin';
import { RichTextEditor } from '../../../components/rich-text-editor';
import { Badge } from '../../../components/ui/badge';
import { Button } from '../../../components/ui/button';
import { Input } from '../../../components/ui/input';
import { Label } from '../../../components/ui/label';
import { Textarea } from '../../../components/ui/textarea';
import { ianaZones } from '../../../village';

// Server-only; the Announcement is null when it is gone or the User may not write content.
const fetchAnnouncement = createServerFn({ method: 'GET' })
  .validator((id: string) => id)
  .handler(async ({ data }) => {
    const response = await getAnnouncement(data);
    return {
      // Typed as JSONContent so the router accepts the body as serializable.
      announcement:
        response.status === 200
          ? { ...response.data, body: response.data.body as JSONContent }
          : null,
      status: response.status,
    };
  });

const saveFn = createServerFn({ method: 'POST' })
  .validator((data: { id: string; announcement: AnnouncementFields }) => data)
  .handler(async ({ data }) => (await updateAnnouncement(data.id, data.announcement)).status);

const publishFn = createServerFn({ method: 'POST' })
  .validator((data: { id: string; publish: boolean }) => data)
  .handler(
    async ({ data }) =>
      (await (data.publish ? publishAnnouncement(data.id) : unpublishAnnouncement(data.id))).status,
  );

const deleteFn = createServerFn({ method: 'POST' })
  .validator((id: string) => id)
  .handler(async ({ data }) => (await deleteAnnouncement(data)).status);

export const Route = createFileRoute('/admin/pengumuman/$id')({
  loader: ({ params }) => fetchAnnouncement({ data: params.id }),
  head: () => ({ meta: [{ title: 'Tulis pengumuman · Pranata' }] }),
  component: Editor,
});

const root = getRouteApi('__root__');

const saveErrors: Record<number, string> = {
  403: 'Anda tidak memiliki izin mengubah pengumuman ini; pengumuman yang sudah terbit hanya dapat diubah oleh penerbit.',
  404: 'Pengumuman ini sudah dihapus.',
  409: 'Slug sudah terkunci sejak pertama terbit, atau baru saja dipakai pengumuman lain.',
  422: 'Periksa lagi isian: judul wajib diisi (paling banyak 150 karakter), ringkasan paling banyak 250 karakter, tanggal berakhir tidak boleh sebelum tanggal mulai, dan pengumuman yang terbit wajib punya ringkasan serta kedua tanggal.',
};

const publishErrors: Record<number, string> = {
  403: 'Hanya pengguna dengan izin menerbitkan konten yang dapat menerbitkan, membatalkan terbit, atau menghapus pengumuman.',
  404: 'Pengumuman ini sudah dihapus.',
  409: 'Status pengumuman sudah berubah; muat ulang halaman.',
  422: 'Sebelum terbit, lengkapi ringkasan, tanggal mulai, dan tanggal berakhir.',
};

function Editor() {
  const { announcement, status } = Route.useLoaderData();
  if (!announcement) {
    return (
      <>
        <PageHead title="Pengumuman" />
        <Notice error>
          {status === 403
            ? 'Anda tidak memiliki izin menulis konten.'
            : status === 404
              ? 'Pengumuman ini sudah dihapus.'
              : 'Gagal memuat pengumuman.'}
        </Notice>
      </>
    );
  }
  // Keyed, so the form starts over from the server's copy when another one opens.
  return <Form key={announcement.id} announcement={announcement} />;
}

function Form({ announcement }: { announcement: Announcement & { body: JSONContent } }) {
  const village = root.useLoaderData();
  const router = useRouter();
  const navigate = useNavigate();
  const save = useServerFn(saveFn);
  const setPublished = useServerFn(publishFn);
  const remove = useServerFn(deleteFn);
  const [message, setMessage] = useState<{ text: string; error: boolean } | null>(null);
  const { fields, change, dirty, saving, savedAt, flush, discard } =
    useAutosave<AnnouncementFields>(
      () => ({
        title: announcement.title,
        slug: announcement.slug ?? null,
        summary: announcement.summary ?? null,
        startDate: announcement.startDate ?? null,
        endDate: announcement.endDate ?? null,
        body: announcement.body,
      }),
      announcement.published,
      (saved) => save({ data: { id: announcement.id, announcement: saved } }),
      (result) =>
        setMessage(
          result >= 300 ? { text: saveErrors[result] ?? 'Gagal menyimpan.', error: true } : null,
        ),
    );
  const locked = Boolean(announcement.firstPublishedAt);
  const time = new Intl.DateTimeFormat('id-ID', {
    timeStyle: 'short',
    timeZone: ianaZones[village?.timeZone ?? 'WIB'],
  });

  async function onPublish(publish: boolean) {
    if (dirty && !announcement.published && !(await flush())) return;
    const result = await setPublished({ data: { id: announcement.id, publish } });
    if (result >= 300) {
      setMessage({ text: publishErrors[result] ?? 'Gagal mengubah status.', error: true });
      return;
    }
    setMessage({
      text: publish ? 'Pengumuman terbit di situs.' : 'Pengumuman kembali menjadi draf.',
      error: false,
    });
    await router.invalidate();
  }

  async function onDelete() {
    if (!window.confirm(`Hapus draf "${announcement.title}" untuk selamanya?`)) return;
    const result = await remove({ data: announcement.id });
    if (result >= 300) {
      setMessage({ text: publishErrors[result] ?? 'Gagal menghapus.', error: true });
      return;
    }
    discard();
    await navigate({ to: '/admin/pengumuman' });
  }

  const status = announcement.published
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
        to="/admin/pengumuman"
        className="flex w-fit items-center gap-1 text-muted-foreground hover:text-foreground"
      >
        <ArrowLeft className="size-4" aria-hidden="true" />
        Semua pengumuman
      </Link>
      <div className="flex flex-wrap items-center gap-3">
        <h1 className="text-xl font-semibold">
          {announcement.published ? 'Ubah pengumuman' : 'Tulis pengumuman'}
        </h1>
        <Badge variant={announcement.published ? 'default' : 'secondary'}>
          {announcement.published ? 'Terbit' : 'Draf'}
        </Badge>
        <p role="status" className="text-muted-foreground">
          {status}
        </p>
      </div>
      {message && <Notice error={message.error}>{message.text}</Notice>}
      <div className="flex flex-wrap gap-2">
        {announcement.published ? (
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
              render={<a href="/pengumuman" target="_blank" rel="noopener" />}
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
              disabled={saving}
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
          <Label htmlFor="announcement-title">Judul</Label>
          <Input
            id="announcement-title"
            required
            maxLength={150}
            value={fields.title}
            onChange={(event) => change({ title: event.currentTarget.value })}
          />
        </div>
        <div className="grid gap-2">
          <Label htmlFor="announcement-slug">Slug</Label>
          <Input
            id="announcement-slug"
            maxLength={80}
            value={fields.slug ?? ''}
            disabled={locked}
            aria-describedby="announcement-slug-hint"
            onChange={(event) => change({ slug: event.currentTarget.value })}
          />
          <p id="announcement-slug-hint" className="text-sm text-muted-foreground">
            {locked
              ? `Terkunci sejak pertama terbit, agar tautan yang sudah dibagikan tidak rusak: /pengumuman/${announcement.slug}`
              : `Alamat halaman lengkapnya, bila ada isi: /pengumuman/${announcement.slug}. Kosongkan agar dibuat dari judul.`}
          </p>
        </div>
        <div className="grid gap-2">
          <Label htmlFor="announcement-summary">Ringkasan</Label>
          <Textarea
            id="announcement-summary"
            maxLength={250}
            rows={3}
            value={fields.summary ?? ''}
            aria-describedby="announcement-summary-hint"
            onChange={(event) => change({ summary: event.currentTarget.value })}
          />
          <p id="announcement-summary-hint" className="text-sm text-muted-foreground">
            Teks biasa yang tampil di beranda dan daftar pengumuman. {(fields.summary ?? '').length}
            /250
          </p>
        </div>
        <fieldset className="grid gap-3">
          <legend className="mb-2 text-sm font-medium">
            Masa berlaku (zona waktu desa, kedua tanggal termasuk)
          </legend>
          <div className="flex flex-wrap gap-4">
            <div className="grid gap-2">
              <Label htmlFor="announcement-start">Mulai</Label>
              <Input
                id="announcement-start"
                type="date"
                value={fields.startDate ?? ''}
                onChange={(event) => change({ startDate: event.currentTarget.value || null })}
              />
            </div>
            <div className="grid gap-2">
              <Label htmlFor="announcement-end">Berakhir</Label>
              <Input
                id="announcement-end"
                type="date"
                min={fields.startDate ?? undefined}
                value={fields.endDate ?? ''}
                onChange={(event) => change({ endDate: event.currentTarget.value || null })}
              />
            </div>
          </div>
        </fieldset>
        <div className="grid gap-2">
          <span id="announcement-body-label" className="text-sm font-medium">
            Isi lengkap (opsional)
          </span>
          <p className="text-sm text-muted-foreground">
            Bila diisi, pengumuman mendapat halaman sendiri dengan tautan "Selengkapnya".
          </p>
          <RichTextEditor
            content={announcement.body ?? { type: 'doc', content: [] }}
            labelledBy="announcement-body-label"
            onChange={(body) => change({ body })}
          />
        </div>
      </div>
    </>
  );
}
