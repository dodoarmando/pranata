import { createFileRoute, Link, useRouter } from '@tanstack/react-router';
import { createServerFn, useServerFn } from '@tanstack/react-start';
import { ArrowDown, ArrowLeft, ArrowUp, Plus } from 'lucide-react';
import { type FormEvent, useState } from 'react';
import {
  type ComplaintTopic,
  type ComplaintTopicFields,
  createComplaintTopic,
  deleteComplaintTopic,
  listComplaintTopics,
  orderComplaintTopics,
  updateComplaintTopic,
} from '../../../api/generated';
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
import { moved } from '../../../order';

// Server-only; the Topics, or the status that refused them.
const fetchTopics = createServerFn({ method: 'GET' }).handler(async () => {
  const response = await listComplaintTopics();
  return response.status === 200 ? response.data : response.status;
});

const saveFn = createServerFn({ method: 'POST' })
  .validator((data: { id: string | undefined; topic: ComplaintTopicFields }) => data)
  .handler(
    async ({ data }) =>
      (data.id
        ? await updateComplaintTopic(data.id, data.topic)
        : await createComplaintTopic(data.topic)
      ).status,
  );

const deleteFn = createServerFn({ method: 'POST' })
  .validator((id: string) => id)
  .handler(async ({ data }) => (await deleteComplaintTopic(data)).status);

const orderFn = createServerFn({ method: 'POST' })
  .validator((ids: string[]) => ids)
  .handler(async ({ data }) => (await orderComplaintTopics({ ids: data })).status);

export const Route = createFileRoute('/admin/pengaduan/topik')({
  loader: () => fetchTopics(),
  head: () => ({ meta: [{ title: 'Topik pengaduan · Pranata' }] }),
  component: Topics,
});

const errors: Record<number, string> = {
  403: 'Anda tidak memiliki izin menangani pengaduan.',
  404: 'Topik ini sudah dihapus.',
  409: 'Tidak dapat disimpan: nama itu sudah dipakai, topik sudah dipakai pengaduan (nonaktifkan saja), atau ini topik aktif terakhir.',
  422: 'Nama wajib diisi, paling banyak 50 karakter, dan urutan memuat setiap topik.',
};

/** What the dialog edits: a new Topic, or an existing one. */
type Editing = { topic?: ComplaintTopic };

function Topics() {
  const topics = Route.useLoaderData();
  const router = useRouter();
  const save = useServerFn(saveFn);
  const remove = useServerFn(deleteFn);
  const order = useServerFn(orderFn);
  const [message, setMessage] = useState<{ text: string; error: boolean } | null>(null);
  const [editing, setEditing] = useState<Editing | null>(null);
  const [dialogError, setDialogError] = useState('');

  const back = (
    <Link
      to="/admin/pengaduan"
      className="flex w-fit items-center gap-1.5 text-muted-foreground hover:text-foreground"
    >
      <ArrowLeft className="size-4" aria-hidden="true" />
      Semua pengaduan
    </Link>
  );
  if (typeof topics === 'number') {
    return (
      <>
        {back}
        <PageHead title="Topik pengaduan" />
        <Notice error>{errors[topics] ?? 'Gagal memuat topik.'}</Notice>
      </>
    );
  }

  async function done(status: number, text: string) {
    setMessage(
      status < 300
        ? { text, error: false }
        : { text: errors[status] ?? 'Gagal menyimpan.', error: true },
    );
    if (status < 300) await router.invalidate();
  }

  async function onSave(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!editing) return;
    const form = new FormData(event.currentTarget);
    const topic = { name: String(form.get('name') ?? ''), active: form.has('active') };
    const status = await save({ data: { id: editing.topic?.id, topic } });
    if (status >= 300) {
      setDialogError(errors[status] ?? 'Gagal menyimpan.');
      return;
    }
    setEditing(null);
    await done(status, 'Topik tersimpan.');
  }

  async function onMove(index: number, by: -1 | 1) {
    if (typeof topics === 'number') return;
    const ids = moved(
      topics.map((topic) => topic.id),
      index,
      index + by,
    );
    await done(await order({ data: ids }), 'Urutan tersimpan.');
  }

  const open = (next: Editing) => {
    setDialogError('');
    setEditing(next);
  };

  return (
    <>
      {back}
      <PageHead title="Topik pengaduan">
        Topik aktif dapat dipilih warga di formulir pengaduan, dalam urutan ini. Topik yang sudah
        dipakai hanya dapat dinonaktifkan, tidak dihapus.
      </PageHead>
      {message && <Notice error={message.error}>{message.text}</Notice>}
      <Button type="button" size="lg" className="w-fit px-4" onClick={() => open({})}>
        <Plus aria-hidden="true" />
        Tambah topik
      </Button>
      <DataTable>
        <TableHeader>
          <TableRow>
            <TableHead>Nama</TableHead>
            <TableHead>Status</TableHead>
            <TableHead>Tindakan</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {topics.map((topic, index) => (
            <TableRow key={topic.id}>
              <TableCell className="font-medium">{topic.name}</TableCell>
              <TableCell>
                <Badge variant={topic.active ? 'secondary' : 'outline'}>
                  {topic.active ? 'Aktif' : 'Nonaktif'}
                </Badge>
              </TableCell>
              <TableCell>
                <div className="flex gap-2">
                  <Button
                    type="button"
                    variant="outline"
                    size="icon"
                    aria-label={`Naikkan ${topic.name}`}
                    disabled={index === 0}
                    onClick={() => onMove(index, -1)}
                  >
                    <ArrowUp aria-hidden="true" />
                  </Button>
                  <Button
                    type="button"
                    variant="outline"
                    size="icon"
                    aria-label={`Turunkan ${topic.name}`}
                    disabled={index === topics.length - 1}
                    onClick={() => onMove(index, 1)}
                  >
                    <ArrowDown aria-hidden="true" />
                  </Button>
                  <Button type="button" variant="outline" onClick={() => open({ topic })}>
                    Ubah
                  </Button>
                  {!topic.used && (
                    <Button
                      type="button"
                      variant="destructive"
                      onClick={async () => done(await remove({ data: topic.id }), 'Topik dihapus.')}
                    >
                      Hapus
                    </Button>
                  )}
                </div>
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </DataTable>
      <Dialog open={editing !== null} onOpenChange={(isOpen) => !isOpen && setEditing(null)}>
        <DialogContent>
          {editing && (
            <form onSubmit={onSave} className="grid gap-4">
              <DialogHeader>
                <DialogTitle>
                  {editing.topic ? `Ubah ${editing.topic.name}` : 'Tambah topik'}
                </DialogTitle>
              </DialogHeader>
              <div className="grid gap-2">
                <Label htmlFor="topic-name">Nama</Label>
                <Input
                  id="topic-name"
                  name="name"
                  required
                  maxLength={50}
                  defaultValue={editing.topic?.name ?? ''}
                />
              </div>
              <label className="flex items-start gap-2.5">
                <input
                  type="checkbox"
                  name="active"
                  defaultChecked={editing.topic?.active ?? true}
                  className="mt-0.5 size-4 accent-primary"
                />
                Aktif, dapat dipilih di formulir pengaduan
              </label>
              {dialogError && <Notice error>{dialogError}</Notice>}
              <DialogFooter>
                <DialogClose
                  render={<Button type="button" variant="outline" size="lg" className="px-4" />}
                >
                  Batal
                </DialogClose>
                <Button type="submit" size="lg" className="px-4">
                  Simpan
                </Button>
              </DialogFooter>
            </form>
          )}
        </DialogContent>
      </Dialog>
    </>
  );
}
