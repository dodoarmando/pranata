import { createFileRoute, getRouteApi, Link, useNavigate, useRouter } from '@tanstack/react-router';
import { createServerFn, useServerFn } from '@tanstack/react-start';
import { ArrowLeft } from 'lucide-react';
import { useState } from 'react';
import {
  type Category,
  deleteEvent,
  type Event,
  type EventFields,
  getEvent,
  listCategories,
  publishEvent,
  unpublishEvent,
  updateEvent,
} from '../../../api/generated';
import { useAutosave } from '../../../autosave';
import { Notice, PageHead } from '../../../components/admin';
import { Badge } from '../../../components/ui/badge';
import { Button } from '../../../components/ui/button';
import { Input } from '../../../components/ui/input';
import { Label } from '../../../components/ui/label';
import { NativeSelect, NativeSelectOption } from '../../../components/ui/native-select';
import { Textarea } from '../../../components/ui/textarea';
import { ianaZones } from '../../../village';

// Server-only; the Event is null when it is gone or the User may not write content.
const fetchEvent = createServerFn({ method: 'GET' })
  .validator((id: string) => id)
  .handler(async ({ data }) => {
    const [event, categories] = await Promise.all([getEvent(data), listCategories()]);
    return {
      event: event.status === 200 ? event.data : null,
      status: event.status,
      categories: categories.status === 200 ? categories.data : [],
    };
  });

const saveFn = createServerFn({ method: 'POST' })
  .validator((data: { id: string; event: EventFields }) => data)
  .handler(async ({ data }) => (await updateEvent(data.id, data.event)).status);

const publishFn = createServerFn({ method: 'POST' })
  .validator((data: { id: string; publish: boolean }) => data)
  .handler(
    async ({ data }) =>
      (await (data.publish ? publishEvent(data.id) : unpublishEvent(data.id))).status,
  );

const deleteFn = createServerFn({ method: 'POST' })
  .validator((id: string) => id)
  .handler(async ({ data }) => (await deleteEvent(data)).status);

export const Route = createFileRoute('/admin/agenda/$id')({
  loader: ({ params }) => fetchEvent({ data: params.id }),
  head: () => ({ meta: [{ title: 'Tulis kegiatan · Pranata' }] }),
  component: Editor,
});

const root = getRouteApi('__root__');

const saveErrors: Record<number, string> = {
  403: 'Anda tidak memiliki izin mengubah kegiatan ini; kegiatan yang sudah terbit hanya dapat diubah oleh penerbit.',
  404: 'Kegiatan ini sudah dihapus.',
  422: 'Periksa lagi isian: judul wajib diisi (paling banyak 150 karakter), tempat paling banyak 200 karakter, keterangan paling banyak 2000 karakter, waktu selesai tidak boleh sebelum waktu mulai, dan kegiatan yang terbit wajib punya kategori, waktu mulai, serta tempat.',
};

const publishErrors: Record<number, string> = {
  403: 'Hanya pengguna dengan izin menerbitkan konten yang dapat menerbitkan, membatalkan terbit, atau menghapus kegiatan.',
  404: 'Kegiatan ini sudah dihapus.',
  409: 'Status kegiatan sudah berubah; muat ulang halaman.',
  422: 'Sebelum terbit, lengkapi kategori, waktu mulai, dan tempat.',
};

// The API speaks seconds; a datetime-local field, minutes.
const toInput = (dateTime: string | null | undefined) => dateTime?.slice(0, 16) ?? '';
const fromInput = (value: string) => (value ? `${value}:00` : null);

function Editor() {
  const { event, status, categories } = Route.useLoaderData();
  if (!event) {
    return (
      <>
        <PageHead title="Agenda" />
        <Notice error>
          {status === 403
            ? 'Anda tidak memiliki izin menulis konten.'
            : status === 404
              ? 'Kegiatan ini sudah dihapus.'
              : 'Gagal memuat kegiatan.'}
        </Notice>
      </>
    );
  }
  // Keyed, so the form starts over from the server's copy when another one opens.
  return <Form key={event.id} event={event} categories={categories} />;
}

function Form({ event, categories }: { event: Event; categories: Category[] }) {
  const village = root.useLoaderData();
  const router = useRouter();
  const navigate = useNavigate();
  const save = useServerFn(saveFn);
  const setPublished = useServerFn(publishFn);
  const remove = useServerFn(deleteFn);
  const [message, setMessage] = useState<{ text: string; error: boolean } | null>(null);
  const { fields, change, dirty, saving, savedAt, flush, discard } = useAutosave<EventFields>(
    () => ({
      title: event.title,
      category: event.category ?? null,
      startsAt: event.startsAt ?? null,
      endsAt: event.endsAt ?? null,
      location: event.location ?? null,
      description: event.description ?? null,
      cancelled: event.cancelled ?? false,
    }),
    event.published,
    (saved) => save({ data: { id: event.id, event: saved } }),
    (result) =>
      setMessage(
        result >= 300 ? { text: saveErrors[result] ?? 'Gagal menyimpan.', error: true } : null,
      ),
  );
  const zone = village?.timeZone ?? 'WIB';
  const time = new Intl.DateTimeFormat('id-ID', { timeStyle: 'short', timeZone: ianaZones[zone] });

  async function onPublish(publish: boolean) {
    if (dirty && !event.published && !(await flush())) return;
    const result = await setPublished({ data: { id: event.id, publish } });
    if (result >= 300) {
      setMessage({ text: publishErrors[result] ?? 'Gagal mengubah status.', error: true });
      return;
    }
    setMessage({
      text: publish ? 'Kegiatan terbit di Agenda.' : 'Kegiatan kembali menjadi draf.',
      error: false,
    });
    await router.invalidate();
  }

  async function onDelete() {
    if (!window.confirm(`Hapus draf "${event.title}" untuk selamanya?`)) return;
    const result = await remove({ data: event.id });
    if (result >= 300) {
      setMessage({ text: publishErrors[result] ?? 'Gagal menghapus.', error: true });
      return;
    }
    discard();
    await navigate({ to: '/admin/agenda' });
  }

  const status = event.published
    ? dirty
      ? 'Ada perubahan yang belum disimpan.'
      : 'Terbit di Agenda.'
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
        to="/admin/agenda"
        className="flex w-fit items-center gap-1 text-muted-foreground hover:text-foreground"
      >
        <ArrowLeft className="size-4" aria-hidden="true" />
        Semua kegiatan
      </Link>
      <div className="flex flex-wrap items-center gap-3">
        <h1 className="text-xl font-semibold">
          {event.published ? 'Ubah kegiatan' : 'Tulis kegiatan'}
        </h1>
        <Badge variant={event.published ? 'default' : 'secondary'}>
          {event.published ? 'Terbit' : 'Draf'}
        </Badge>
        <p role="status" className="text-muted-foreground">
          {status}
        </p>
      </div>
      {message && <Notice error={message.error}>{message.text}</Notice>}
      <div className="flex flex-wrap gap-2">
        {event.published ? (
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
              render={<a href="/agenda" target="_blank" rel="noopener" />}
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
          <Label htmlFor="event-title">Judul</Label>
          <Input
            id="event-title"
            required
            maxLength={150}
            value={fields.title}
            onChange={(e) => change({ title: e.currentTarget.value })}
          />
        </div>
        <div className="grid gap-2">
          <Label htmlFor="event-category">Kategori</Label>
          <NativeSelect
            id="event-category"
            value={fields.category ?? ''}
            onChange={(e) => change({ category: e.currentTarget.value || null })}
          >
            <NativeSelectOption value="">Pilih kategori</NativeSelectOption>
            {categories.map((category) => (
              <NativeSelectOption key={category.id} value={category.id}>
                {category.name}
              </NativeSelectOption>
            ))}
          </NativeSelect>
        </div>
        <fieldset className="grid gap-3">
          <legend className="mb-2 text-sm font-medium">Waktu ({zone})</legend>
          <div className="flex flex-wrap gap-4">
            <div className="grid gap-2">
              <Label htmlFor="event-start">Mulai</Label>
              <Input
                id="event-start"
                type="datetime-local"
                value={toInput(fields.startsAt)}
                onChange={(e) => change({ startsAt: fromInput(e.currentTarget.value) })}
              />
            </div>
            <div className="grid gap-2">
              <Label htmlFor="event-end">Selesai (opsional)</Label>
              <Input
                id="event-end"
                type="datetime-local"
                min={toInput(fields.startsAt) || undefined}
                value={toInput(fields.endsAt)}
                aria-describedby="event-end-hint"
                onChange={(e) => change({ endsAt: fromInput(e.currentTarget.value) })}
              />
            </div>
          </div>
          <p id="event-end-hint" className="text-sm text-muted-foreground">
            Boleh berhari-hari. Tanpa waktu selesai, kegiatan dianggap selesai di akhir hari
            mulainya.
          </p>
        </fieldset>
        <div className="grid gap-2">
          <Label htmlFor="event-location">Tempat</Label>
          <Input
            id="event-location"
            maxLength={200}
            value={fields.location ?? ''}
            onChange={(e) => change({ location: e.currentTarget.value })}
          />
        </div>
        <div className="grid gap-2">
          <Label htmlFor="event-description">Keterangan (opsional)</Label>
          <Textarea
            id="event-description"
            maxLength={2000}
            rows={4}
            value={fields.description ?? ''}
            aria-describedby="event-description-hint"
            onChange={(e) => change({ description: e.currentTarget.value })}
          />
          <p id="event-description-hint" className="text-sm text-muted-foreground">
            Teks biasa. {(fields.description ?? '').length}/2000
          </p>
        </div>
        <label className="flex items-center gap-2.5">
          <input
            type="checkbox"
            checked={fields.cancelled ?? false}
            aria-describedby="event-cancelled-hint"
            onChange={(e) => change({ cancelled: e.currentTarget.checked })}
            className="size-4 accent-primary"
          />
          Dibatalkan
        </label>
        <p id="event-cancelled-hint" className="-mt-3 text-sm text-muted-foreground">
          Kegiatan yang batal tetap tercantum di Agenda dengan label Dibatalkan.
        </p>
      </div>
    </>
  );
}
