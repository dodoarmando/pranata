import { createFileRoute, useRouter } from '@tanstack/react-router';
import { createServerFn, useServerFn } from '@tanstack/react-start';
import { ArrowDown, ArrowUp, Plus } from 'lucide-react';
import { type FormEvent, useState } from 'react';
import {
  createRegionUnit,
  getRegionTree,
  type NewRegionUnit,
  orderHamlets,
  type RegionKind,
  type RegionSettings,
  type RegionUnit,
  type RegionUnitUpdate,
  updateRegionSettings,
  updateRegionUnit,
} from '../../api/generated';
import { DataTable, Notice, PageHead } from '../../components/admin';
import { Badge } from '../../components/ui/badge';
import { Button } from '../../components/ui/button';
import { Card, CardAction, CardContent, CardHeader } from '../../components/ui/card';
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
import { TableBody, TableCell, TableHead, TableHeader, TableRow } from '../../components/ui/table';

// Server-only; the tree, or the status that refused it.
const fetchTree = createServerFn({ method: 'GET' }).handler(async () => {
  const response = await getRegionTree();
  return response.status === 200 ? response.data : response.status;
});

const saveSettingsFn = createServerFn({ method: 'POST' })
  .validator((settings: RegionSettings) => settings)
  .handler(async ({ data }) => (await updateRegionSettings(data)).status);

const createUnitFn = createServerFn({ method: 'POST' })
  .validator((unit: NewRegionUnit) => unit)
  .handler(async ({ data }) => (await createRegionUnit(data)).status);

const updateUnitFn = createServerFn({ method: 'POST' })
  .validator((data: { id: string; unit: RegionUnitUpdate }) => data)
  .handler(async ({ data }) => (await updateRegionUnit(data.id, data.unit)).status);

const orderFn = createServerFn({ method: 'POST' })
  .validator((ids: string[]) => ids)
  .handler(async ({ data }) => (await orderHamlets({ ids: data })).status);

export const Route = createFileRoute('/admin/wilayah')({
  loader: () => fetchTree(),
  head: () => ({ meta: [{ title: 'Wilayah · Pranata' }] }),
  component: Regions,
});

const forbidden = 'Anda tidak memiliki izin mengelola data desa.';

const settingsErrors: Record<number, string> = {
  403: forbidden,
  409: 'Tingkat RW tidak dapat diubah setelah ada RT, dan hanya dapat dimatikan bila semua RW sudah nonaktif.',
  422: 'Sebutan wajib diisi, paling banyak 30 karakter.',
};

const unitErrors: Record<number, string> = {
  403: forbidden,
  409: 'Tidak dapat disimpan: nomor itu sudah dipakai, masih ada wilayah aktif di bawahnya, masa jabatan kepalanya belum diberi tanggal selesai, atau wilayah di atasnya nonaktif.',
  422: 'Periksa lagi isian: nama wajib diisi (paling banyak 100 karakter), nomor 1–999, dan telepon yang sah.',
};

const hamletLabels = ['dusun', 'banjar', 'jorong', 'lingkungan'];

const capitalized = (text: string) => text.charAt(0).toUpperCase() + text.slice(1);

/** What the dialog adds or edits: a new unit under its parent, or an existing one. */
type Editing = { kind: RegionKind; parentId?: string; unit?: RegionUnit };

function Regions() {
  const tree = Route.useLoaderData();
  const router = useRouter();
  const saveSettings = useServerFn(saveSettingsFn);
  const createUnit = useServerFn(createUnitFn);
  const updateUnit = useServerFn(updateUnitFn);
  const order = useServerFn(orderFn);
  const [message, setMessage] = useState<{ text: string; error: boolean } | null>(null);
  const [editing, setEditing] = useState<Editing | null>(null);
  const [dialogError, setDialogError] = useState('');

  if (typeof tree === 'number') {
    return (
      <>
        <PageHead title="Wilayah" />
        <Notice error>Kode Desa belum ditetapkan. Hubungi penyedia Pranata.</Notice>
      </>
    );
  }

  const label = capitalized(tree.hamletLabel);
  const hamlets = tree.units.filter((unit) => unit.kind === 'hamlet');
  const childrenOf = (id: string) => tree.units.filter((unit) => unit.parentId === id);
  const hasRt = tree.units.some((unit) => unit.kind === 'rt');
  const title = (unit: RegionUnit) =>
    unit.kind === 'hamlet'
      ? `${label} ${unit.name}`
      : `${unit.kind.toUpperCase()} ${String(unit.number).padStart(3, '0')}`;

  async function done(status: number, errors: Record<number, string>, text: string) {
    setMessage(
      status < 300
        ? { text, error: false }
        : { text: errors[status] ?? 'Gagal menyimpan.', error: true },
    );
    if (status < 300) await router.invalidate();
  }

  async function onSettings(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const form = new FormData(event.currentTarget);
    const status = await saveSettings({
      data: { hamletLabel: String(form.get('hamletLabel')), hasRw: form.has('hasRw') },
    });
    await done(status, settingsErrors, 'Pengaturan wilayah tersimpan.');
  }

  async function onMove(index: number, by: -1 | 1) {
    const ids = hamlets.map((hamlet) => hamlet.id);
    const [moved] = ids.splice(index, 1);
    if (moved) ids.splice(index + by, 0, moved);
    await done(await order({ data: ids }), unitErrors, 'Urutan tersimpan.');
  }

  async function onSaveUnit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!editing) return;
    const form = new FormData(event.currentTarget);
    const text = (name: string) => String(form.get(name) ?? '').trim() || null;
    const fields =
      editing.kind === 'hamlet'
        ? { name: text('name') }
        : {
            number: Number(form.get('number')),
            headName: text('headName'),
            headPhone: text('headPhone'),
          };
    const status = editing.unit
      ? await updateUnit({
          data: { id: editing.unit.id, unit: { ...fields, active: form.has('active') } },
        })
      : await createUnit({
          data: { ...fields, kind: editing.kind, parentId: editing.parentId ?? null },
        });
    if (status >= 300) {
      setDialogError(unitErrors[status] ?? 'Gagal menyimpan.');
      return;
    }
    setEditing(null);
    await done(status, unitErrors, 'Wilayah tersimpan.');
  }

  const open = (next: Editing) => {
    setDialogError('');
    setEditing(next);
  };

  const status = (unit: RegionUnit) => (
    <Badge variant={unit.active ? 'secondary' : 'outline'}>
      {unit.active ? 'Aktif' : 'Nonaktif'}
    </Badge>
  );

  const row = (unit: RegionUnit, nested = false) => (
    <TableRow key={unit.id}>
      <TableCell className={nested ? 'pl-8' : 'font-medium'}>{title(unit)}</TableCell>
      <TableCell>{unit.headName ?? '—'}</TableCell>
      <TableCell>{unit.headPhone ?? '—'}</TableCell>
      <TableCell>{status(unit)}</TableCell>
      <TableCell>
        <div className="flex gap-2">
          <Button type="button" variant="outline" onClick={() => open({ kind: unit.kind, unit })}>
            Ubah
          </Button>
          {unit.kind === 'rw' && unit.active && tree.hasRw && (
            <Button
              type="button"
              variant="outline"
              onClick={() => open({ kind: 'rt', parentId: unit.id })}
            >
              <Plus aria-hidden="true" />
              RT
            </Button>
          )}
        </div>
      </TableCell>
    </TableRow>
  );

  const editingTitle = editing?.unit
    ? `Ubah ${title(editing.unit)}`
    : `Tambah ${editing?.kind === 'hamlet' ? tree.hamletLabel : editing?.kind.toUpperCase()}`;

  return (
    <>
      <PageHead title="Wilayah">
        {label}, RW, dan RT desa. Ketua RW dan RT tidak tampil di situs publik.
      </PageHead>
      {message && <Notice error={message.error}>{message.text}</Notice>}
      <Card>
        <form onSubmit={onSettings} aria-labelledby="region-settings-title" className="contents">
          <CardHeader>
            <h2 id="region-settings-title" className="text-base font-semibold">
              Pengaturan
            </h2>
          </CardHeader>
          <CardContent className="flex flex-wrap items-end gap-5">
            <div className="grid w-full max-w-xs gap-2">
              <Label htmlFor="hamlet-label">Sebutan wilayah di bawah desa</Label>
              <Input
                id="hamlet-label"
                name="hamletLabel"
                list="hamlet-labels"
                required
                maxLength={30}
                defaultValue={tree.hamletLabel}
              />
              <datalist id="hamlet-labels">
                {hamletLabels.map((option) => (
                  <option key={option} value={option} />
                ))}
              </datalist>
            </div>
            <label className="flex items-center gap-2.5 pb-2">
              <input
                type="checkbox"
                name="hasRw"
                defaultChecked={tree.hasRw}
                disabled={hasRt}
                aria-describedby="has-rw-hint"
                className="size-4 accent-primary"
              />
              Desa memakai tingkat RW
            </label>
            {/* A disabled checkbox is not submitted, so carry its value. */}
            {hasRt && tree.hasRw && <input type="hidden" name="hasRw" value="on" />}
            <p id="has-rw-hint" className="w-full text-muted-foreground">
              {hasRt
                ? 'Tingkat RW tidak dapat diubah lagi karena sudah ada RT.'
                : 'Tentukan sebelum menambah RT; tidak dapat diubah setelah ada RT. Nonaktifkan semua RW sebelum mematikannya.'}
            </p>
            <Button type="submit" size="lg" className="px-4">
              Simpan pengaturan
            </Button>
          </CardContent>
        </form>
      </Card>
      {hamlets.map((hamlet, index) => {
        const children = childrenOf(hamlet.id);
        return (
          <Card key={hamlet.id} role="region" aria-labelledby={`hamlet-${hamlet.id}`}>
            <CardHeader>
              <h2 id={`hamlet-${hamlet.id}`} className="text-base font-semibold">
                {title(hamlet)} {!hamlet.active && status(hamlet)}
              </h2>
              <CardAction className="flex gap-2">
                <Button
                  type="button"
                  variant="outline"
                  size="icon"
                  aria-label={`Naikkan ${title(hamlet)}`}
                  disabled={index === 0}
                  onClick={() => onMove(index, -1)}
                >
                  <ArrowUp aria-hidden="true" />
                </Button>
                <Button
                  type="button"
                  variant="outline"
                  size="icon"
                  aria-label={`Turunkan ${title(hamlet)}`}
                  disabled={index === hamlets.length - 1}
                  onClick={() => onMove(index, 1)}
                >
                  <ArrowDown aria-hidden="true" />
                </Button>
                <Button
                  type="button"
                  variant="outline"
                  onClick={() => open({ kind: 'hamlet', unit: hamlet })}
                >
                  Ubah
                </Button>
              </CardAction>
            </CardHeader>
            <CardContent className="grid gap-4">
              {children.length > 0 && (
                <DataTable>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Wilayah</TableHead>
                      <TableHead>Ketua</TableHead>
                      <TableHead>Telepon</TableHead>
                      <TableHead>Status</TableHead>
                      <TableHead>Tindakan</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {children.flatMap((child) => [
                      row(child),
                      ...childrenOf(child.id).map((rt) => row(rt, true)),
                    ])}
                  </TableBody>
                </DataTable>
              )}
              {hamlet.active && (
                <Button
                  type="button"
                  variant="outline"
                  className="w-fit"
                  onClick={() => open({ kind: tree.hasRw ? 'rw' : 'rt', parentId: hamlet.id })}
                >
                  <Plus aria-hidden="true" />
                  Tambah {tree.hasRw ? 'RW' : 'RT'}
                </Button>
              )}
            </CardContent>
          </Card>
        );
      })}
      <Button
        type="button"
        size="lg"
        className="w-fit px-4"
        onClick={() => open({ kind: 'hamlet' })}
      >
        <Plus aria-hidden="true" />
        Tambah {tree.hamletLabel}
      </Button>
      <Dialog open={editing !== null} onOpenChange={(isOpen) => !isOpen && setEditing(null)}>
        <DialogContent>
          {editing && (
            <form onSubmit={onSaveUnit} className="grid gap-4">
              <DialogHeader>
                <DialogTitle>{editingTitle}</DialogTitle>
              </DialogHeader>
              {editing.kind === 'hamlet' ? (
                <div className="grid gap-2">
                  <Label htmlFor="unit-name">Nama {tree.hamletLabel}</Label>
                  <Input
                    id="unit-name"
                    name="name"
                    required
                    maxLength={100}
                    defaultValue={editing.unit?.name ?? ''}
                  />
                </div>
              ) : (
                <>
                  <div className="grid gap-2">
                    <Label htmlFor="unit-number">Nomor {editing.kind.toUpperCase()}</Label>
                    <Input
                      id="unit-number"
                      name="number"
                      type="number"
                      required
                      min={1}
                      max={999}
                      defaultValue={editing.unit?.number ?? ''}
                    />
                  </div>
                  <div className="grid gap-2">
                    <Label htmlFor="unit-head-name">Nama ketua (opsional)</Label>
                    <Input
                      id="unit-head-name"
                      name="headName"
                      maxLength={100}
                      defaultValue={editing.unit?.headName ?? ''}
                    />
                  </div>
                  <div className="grid gap-2">
                    <Label htmlFor="unit-head-phone">Telepon ketua (opsional)</Label>
                    <Input
                      id="unit-head-phone"
                      name="headPhone"
                      type="tel"
                      autoComplete="off"
                      maxLength={20}
                      defaultValue={editing.unit?.headPhone ?? ''}
                    />
                  </div>
                </>
              )}
              {editing.unit && (
                <label className="flex items-start gap-2.5">
                  <input
                    type="checkbox"
                    name="active"
                    defaultChecked={editing.unit.active}
                    aria-describedby="unit-active-hint"
                    className="mt-0.5 size-4 accent-primary"
                  />
                  <span>
                    Aktif
                    <span id="unit-active-hint" className="block text-muted-foreground">
                      Hanya dapat dinonaktifkan bila semua wilayah di bawahnya sudah nonaktif
                      {editing.kind === 'hamlet' &&
                        ' dan masa jabatan kepalanya sudah diberi tanggal selesai di Pemerintahan'}
                      . Wilayah nonaktif tidak dapat dipilih untuk data baru, dan tidak pernah
                      dihapus.
                    </span>
                  </span>
                </label>
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
