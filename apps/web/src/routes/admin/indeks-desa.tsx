import { createFileRoute, useRouter } from '@tanstack/react-router';
import { createServerFn, useServerFn } from '@tanstack/react-start';
import { Plus } from 'lucide-react';
import { type FormEvent, useState } from 'react';
import {
  createIndexEdition,
  type DevelopmentStatus,
  deleteIndexEdition,
  type IndexEdition,
  type IndexEditionFields,
  type IndexKind,
  listIndexEditions,
  updateIndexEdition,
} from '../../api/generated';
import { DataTable, Notice, PageHead } from '../../components/admin';
import { Badge } from '../../components/ui/badge';
import { Button } from '../../components/ui/button';
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '../../components/ui/dialog';
import { Input } from '../../components/ui/input';
import { Label } from '../../components/ui/label';
import { NativeSelect, NativeSelectOption } from '../../components/ui/native-select';
import {
  TableBody,
  TableCaption,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '../../components/ui/table';
import { dimensions, editionLabel, formatScore, scales, statusLabels } from '../../index-edition';

// Server-only; the Editions, or null when the API cannot be reached.
const fetchEditions = createServerFn({ method: 'GET' }).handler(async () => {
  const response = await listIndexEditions();
  return response.status === 200 ? response.data : null;
});

const createEditionFn = createServerFn({ method: 'POST' })
  .validator((edition: IndexEditionFields) => edition)
  .handler(async ({ data }) => (await createIndexEdition(data)).status);

const updateEditionFn = createServerFn({ method: 'POST' })
  .validator((data: { id: string; edition: IndexEditionFields }) => data)
  .handler(async ({ data }) => (await updateIndexEdition(data.id, data.edition)).status);

const deleteEditionFn = createServerFn({ method: 'POST' })
  .validator((id: string) => id)
  .handler(async ({ data }) => (await deleteIndexEdition(data)).status);

export const Route = createFileRoute('/admin/indeks-desa')({
  loader: () => fetchEditions(),
  head: () => ({ meta: [{ title: 'Indeks Desa · Pranata' }] }),
  component: Editions,
});

const saveErrors: Record<number, string> = {
  403: 'Anda tidak memiliki izin mengelola data desa.',
  404: 'Edisi ini sudah dihapus.',
  409: 'Tahun ini sudah punya edisi.',
  422: 'Periksa lagi isian: tahun 2000–2100, skor Indeks Desa dan dimensinya 0–100 dengan paling banyak 2 desimal, skor IDM 0–1 dengan paling banyak 4 desimal.',
};

function Editions() {
  const editions = Route.useLoaderData();
  const router = useRouter();
  const saveNew = useServerFn(createEditionFn);
  const save = useServerFn(updateEditionFn);
  const remove = useServerFn(deleteEditionFn);
  const [message, setMessage] = useState<{ text: string; error: boolean } | null>(null);
  // The Edition being edited, or null for a new one; undefined while the dialog is closed.
  const [editing, setEditing] = useState<IndexEdition | null | undefined>(undefined);
  const [kind, setKind] = useState<IndexKind>('indeks_desa');
  const [dialogError, setDialogError] = useState('');

  if (!editions) {
    return (
      <>
        <PageHead title="Indeks Desa" />
        <Notice error>Gagal memuat edisi Indeks Desa.</Notice>
      </>
    );
  }

  const open = (edition: IndexEdition | null) => {
    setDialogError('');
    setKind(edition?.kind ?? 'indeks_desa');
    setEditing(edition);
  };

  async function onSave(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (editing === undefined) return;
    const form = new FormData(event.currentTarget);
    const number = (name: string) => {
      const value = String(form.get(name) ?? '').trim();
      return value === '' ? null : Number(value);
    };
    const fields: IndexEditionFields = {
      year: number('year') ?? 0,
      kind,
      score: number('score') ?? 0,
      status: form.get('status') as DevelopmentStatus,
      ...Object.fromEntries(dimensions.map(([key]) => [key, kind === 'idm' ? null : number(key)])),
    };
    const status = editing
      ? await save({ data: { id: editing.id, edition: fields } })
      : await saveNew({ data: fields });
    if (status >= 300) {
      setDialogError(saveErrors[status] ?? 'Gagal menyimpan.');
      return;
    }
    setEditing(undefined);
    setMessage({ text: 'Edisi tersimpan.', error: false });
    await router.invalidate();
  }

  async function onDelete(edition: IndexEdition) {
    if (!window.confirm(`Hapus edisi ${editionLabel(edition)}?`)) return;
    const status = await remove({ data: edition.id });
    if (status >= 300) {
      setMessage({ text: saveErrors[status] ?? 'Gagal menghapus.', error: true });
      return;
    }
    setMessage({ text: 'Edisi dihapus.', error: false });
    await router.invalidate();
  }

  return (
    <>
      <PageHead title="Indeks Desa">
        Skor dan status dari kementerian, disimpan seperti yang diumumkan. Edisi dinamai menurut
        tahun pengumpulan data; tahun sebelum 2024 memakai Indeks Desa Membangun (IDM).
      </PageHead>
      {message && <Notice error={message.error}>{message.text}</Notice>}
      {editions.length === 0 ? (
        <p className="text-muted-foreground">Belum ada edisi.</p>
      ) : (
        <DataTable>
          <TableCaption className="sr-only">Edisi Indeks Desa</TableCaption>
          <TableHeader>
            <TableRow>
              <TableHead>Tahun</TableHead>
              <TableHead>Skor</TableHead>
              <TableHead>Status</TableHead>
              <TableHead>
                <span className="sr-only">Tindakan</span>
              </TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {editions.map((edition) => (
              <TableRow key={edition.id}>
                <TableCell>{editionLabel(edition)}</TableCell>
                <TableCell className="tabular-nums">
                  {formatScore(edition.score, edition.kind)}
                </TableCell>
                <TableCell>
                  <Badge variant={edition.kind === 'idm' ? 'secondary' : 'default'}>
                    {statusLabels[edition.status]}
                  </Badge>
                </TableCell>
                <TableCell className="text-right">
                  <div className="flex justify-end gap-2">
                    <Button type="button" variant="outline" onClick={() => open(edition)}>
                      Ubah
                    </Button>
                    <Button type="button" variant="outline" onClick={() => onDelete(edition)}>
                      Hapus
                    </Button>
                  </div>
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </DataTable>
      )}
      <Button type="button" size="lg" className="w-fit px-4" onClick={() => open(null)}>
        <Plus aria-hidden="true" />
        Tambah edisi
      </Button>
      <Dialog
        open={editing !== undefined}
        onOpenChange={(isOpen) => !isOpen && setEditing(undefined)}
      >
        <DialogContent className="max-h-[90dvh] overflow-y-auto sm:max-w-xl">
          {editing !== undefined && (
            <form onSubmit={onSave} className="grid gap-4">
              <DialogHeader>
                <DialogTitle>
                  {editing ? `Ubah edisi ${editionLabel(editing)}` : 'Tambah edisi'}
                </DialogTitle>
              </DialogHeader>
              <div className="grid gap-4 sm:grid-cols-2">
                <div className="grid gap-2">
                  <Label htmlFor="edition-year">Tahun pengumpulan data</Label>
                  <Input
                    id="edition-year"
                    name="year"
                    type="number"
                    required
                    min={2000}
                    max={2100}
                    defaultValue={editing?.year ?? ''}
                  />
                </div>
                <div className="grid gap-2">
                  <Label htmlFor="edition-kind">Indeks</Label>
                  <NativeSelect
                    id="edition-kind"
                    className="w-full"
                    value={kind}
                    onChange={(event) => setKind(event.currentTarget.value as IndexKind)}
                  >
                    <NativeSelectOption value="indeks_desa">Indeks Desa (0–100)</NativeSelectOption>
                    <NativeSelectOption value="idm">IDM (0–1)</NativeSelectOption>
                  </NativeSelect>
                </div>
                <div className="grid gap-2">
                  <Label htmlFor="edition-score">Skor</Label>
                  <Input
                    // A new key resets the value when the scale changes.
                    key={kind}
                    id="edition-score"
                    name="score"
                    type="number"
                    required
                    min={0}
                    max={scales[kind].max}
                    step={10 ** -scales[kind].digits}
                    defaultValue={editing?.kind === kind ? editing.score : ''}
                  />
                </div>
                <div className="grid gap-2">
                  <Label htmlFor="edition-status">Status</Label>
                  <NativeSelect
                    id="edition-status"
                    name="status"
                    className="w-full"
                    required
                    defaultValue={editing?.status ?? ''}
                  >
                    <NativeSelectOption value="" disabled>
                      Pilih status
                    </NativeSelectOption>
                    {(Object.keys(statusLabels) as DevelopmentStatus[]).map((status) => (
                      <NativeSelectOption key={status} value={status}>
                        {statusLabels[status]}
                      </NativeSelectOption>
                    ))}
                  </NativeSelect>
                </div>
              </div>
              {kind === 'indeks_desa' && (
                <fieldset className="grid gap-4 sm:grid-cols-2">
                  <legend className="mb-2 text-sm font-medium">
                    Skor per dimensi (opsional, 0–100)
                  </legend>
                  {dimensions.map(([key, label]) => (
                    <div key={key} className="grid gap-2">
                      <Label htmlFor={`edition-${key}`}>{label}</Label>
                      <Input
                        id={`edition-${key}`}
                        name={key}
                        type="number"
                        min={0}
                        max={100}
                        step={0.01}
                        defaultValue={editing?.[key] ?? ''}
                      />
                    </div>
                  ))}
                </fieldset>
              )}
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
