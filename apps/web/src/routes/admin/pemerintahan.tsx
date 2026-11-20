import { createFileRoute, useRouter } from '@tanstack/react-router';
import { createServerFn, useServerFn } from '@tanstack/react-start';
import { Plus } from 'lucide-react';
import { type ChangeEvent, type FormEvent, useState } from 'react';
import {
  createPosition,
  createTenure,
  deletePosition,
  listPositions,
  type NewTenure,
  type Position,
  renamePosition,
  type Tenure,
  type TenureFields,
  type TenureKind,
  updateTenure,
  uploadTenurePhoto,
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
import { NativeSelect, NativeSelectOption } from '../../components/ui/native-select';
import { TableBody, TableCell, TableHead, TableHeader, TableRow } from '../../components/ui/table';
import { formatDate, tenureKindLabels } from '../../government';

// Server-only; the Positions, or the status that refused them.
const fetchPositions = createServerFn({ method: 'GET' }).handler(async () => {
  const response = await listPositions();
  return response.status === 200 ? response.data : response.status;
});

const createPositionFn = createServerFn({ method: 'POST' })
  .validator((name: string) => name)
  .handler(async ({ data }) => (await createPosition({ name: data })).status);

const renamePositionFn = createServerFn({ method: 'POST' })
  .validator((data: { id: string; name: string }) => data)
  .handler(async ({ data }) => (await renamePosition(data.id, { name: data.name })).status);

const deletePositionFn = createServerFn({ method: 'POST' })
  .validator((id: string) => id)
  .handler(async ({ data }) => (await deletePosition(data)).status);

const createTenureFn = createServerFn({ method: 'POST' })
  .validator((tenure: NewTenure) => tenure)
  .handler(async ({ data }) => (await createTenure(data)).status);

const updateTenureFn = createServerFn({ method: 'POST' })
  .validator((data: { id: string; tenure: TenureFields }) => data)
  .handler(async ({ data }) => (await updateTenure(data.id, data.tenure)).status);

const uploadPhotoFn = createServerFn({ method: 'POST' })
  .validator((data: FormData) => {
    const photo = data.get('photo');
    if (!(photo instanceof File)) throw new Error('Expected a photo file');
    return photo;
  })
  .handler(async ({ data }) => {
    const response = await uploadTenurePhoto(data);
    return response.status === 201
      ? { status: response.status, id: response.data.id }
      : { status: response.status };
  });

export const Route = createFileRoute('/admin/pemerintahan')({
  loader: () => fetchPositions(),
  head: () => ({ meta: [{ title: 'Pemerintahan · Pranata' }] }),
  component: Government,
});

const MAX_PHOTO_BYTES = 10 << 20;

const forbidden = 'Anda tidak memiliki izin mengelola data desa.';

const positionErrors: Record<number, string> = {
  403: forbidden,
  409: 'Jabatan Kepala Desa, jabatan kepala wilayah, dan jabatan yang pernah diisi tidak dapat dihapus atau diganti namanya di sini.',
  422: 'Nama jabatan wajib diisi, paling banyak 100 karakter.',
};

const tenureErrors: Record<number, string> = {
  403: forbidden,
  409: 'Tidak dapat disimpan: sudah ada pejabat Definitif atau Pj. pada rentang tanggal itu, atau jabatannya nonaktif sehingga masa jabatan wajib berakhir.',
  422: 'Periksa lagi isian: nama dan nomor SK wajib diisi, tanggal selesai tidak sebelum tanggal mulai, dan foto wajib punya teks alternatif.',
};

const photoErrors: Record<number, string> = {
  403: forbidden,
  413: 'Foto paling besar 10 MB.',
  415: 'Foto harus berupa berkas JPEG, PNG, atau WebP.',
  422: 'Gambar rusak atau lebih dari 40 megapiksel.',
};

/** What the Tenure dialog adds or edits: a new Tenure in a Position, or an existing one. */
type EditingTenure = { position: Position; tenure?: Tenure };

/** What the Position dialog adds or renames. */
type EditingPosition = { position?: Position };

const period = (tenure: Tenure) =>
  `${formatDate(tenure.startDate)} – ${tenure.endDate ? formatDate(tenure.endDate) : 'sekarang'}`;

function Government() {
  const positions = Route.useLoaderData();
  const router = useRouter();
  const savePositionNew = useServerFn(createPositionFn);
  const savePositionName = useServerFn(renamePositionFn);
  const removePosition = useServerFn(deletePositionFn);
  const saveTenureNew = useServerFn(createTenureFn);
  const saveTenure = useServerFn(updateTenureFn);
  const uploadPhoto = useServerFn(uploadPhotoFn);
  const [message, setMessage] = useState<{ text: string; error: boolean } | null>(null);
  const [editingPosition, setEditingPosition] = useState<EditingPosition | null>(null);
  const [editingTenure, setEditingTenure] = useState<EditingTenure | null>(null);
  const [officialId, setOfficialId] = useState('');
  const [photo, setPhoto] = useState<string | null>(null);
  const [uploading, setUploading] = useState(false);
  const [dialogError, setDialogError] = useState('');

  if (typeof positions === 'number') {
    return (
      <>
        <PageHead title="Pemerintahan" />
        <Notice error>Gagal memuat jabatan.</Notice>
      </>
    );
  }

  // Each Official once; every Tenure of theirs carries the same name and NIPD.
  const officials = [
    ...new Map(
      positions
        .flatMap((position) => position.tenures)
        .map((tenure) => [tenure.officialId, tenure] as const),
    ).values(),
  ].sort((a, b) => a.name.localeCompare(b.name, 'id'));

  async function done(status: number, text: string) {
    setMessage({ text, error: false });
    if (status < 300) await router.invalidate();
  }

  const openTenure = (next: EditingTenure) => {
    setDialogError('');
    setOfficialId(next.tenure?.officialId ?? '');
    setPhoto(next.tenure?.photo ?? null);
    setEditingTenure(next);
  };

  const openPosition = (next: EditingPosition) => {
    setDialogError('');
    setEditingPosition(next);
  };

  async function onSavePosition(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!editingPosition) return;
    const name = String(new FormData(event.currentTarget).get('name') ?? '');
    const status = editingPosition.position
      ? await savePositionName({ data: { id: editingPosition.position.id, name } })
      : await savePositionNew({ data: name });
    if (status >= 300) {
      setDialogError(positionErrors[status] ?? 'Gagal menyimpan.');
      return;
    }
    setEditingPosition(null);
    await done(status, 'Jabatan tersimpan.');
  }

  async function onDeletePosition(position: Position) {
    if (!window.confirm(`Hapus jabatan ${position.name}?`)) return;
    const status = await removePosition({ data: position.id });
    if (status >= 300) {
      setMessage({ text: positionErrors[status] ?? 'Gagal menghapus.', error: true });
      return;
    }
    await done(status, 'Jabatan dihapus.');
  }

  async function onPhoto(event: ChangeEvent<HTMLInputElement>) {
    const file = event.currentTarget.files?.[0];
    event.currentTarget.value = '';
    if (!file) return;
    if (file.size > MAX_PHOTO_BYTES) {
      setDialogError(photoErrors[413] ?? '');
      return;
    }
    setUploading(true);
    const data = new FormData();
    data.set('photo', file);
    const result = await uploadPhoto({ data }).finally(() => setUploading(false));
    if (result.id) {
      setPhoto(result.id);
      setDialogError('');
    } else {
      setDialogError(photoErrors[result.status] ?? 'Gagal mengunggah foto.');
    }
  }

  async function onSaveTenure(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!editingTenure) return;
    const form = new FormData(event.currentTarget);
    const text = (name: string) => String(form.get(name) ?? '').trim() || null;
    const fields: TenureFields = {
      name: text('name') ?? '',
      nipd: text('nipd'),
      kind: String(form.get('kind')) as TenureKind,
      startDate: String(form.get('startDate')),
      endDate: text('endDate'),
      decreeNumber: text('decreeNumber') ?? '',
      decreeDate: String(form.get('decreeDate')),
      photo,
      photoAlt: photo ? text('photoAlt') : null,
    };
    const status = editingTenure.tenure
      ? await saveTenure({ data: { id: editingTenure.tenure.id, tenure: fields } })
      : await saveTenureNew({
          data: {
            ...fields,
            positionId: editingTenure.position.id,
            officialId: officialId || null,
          },
        });
    if (status >= 300) {
      setDialogError(tenureErrors[status] ?? 'Gagal menyimpan.');
      return;
    }
    setEditingTenure(null);
    await done(status, 'Masa jabatan tersimpan.');
  }

  const tenure = editingTenure?.tenure;
  // The fields start from the chosen Official, or from the Tenure being edited.
  const prefill = tenure ?? officials.find((official) => official.officialId === officialId);

  return (
    <>
      <PageHead title="Pemerintahan">
        Jabatan perangkat desa dan masa jabatan pejabatnya. NIPD dan SK tidak tampil di situs
        publik; masa jabatan tidak pernah dihapus.
      </PageHead>
      {message && <Notice error={message.error}>{message.text}</Notice>}
      {positions.map((position) => (
        <Card key={position.id} role="region" aria-labelledby={`position-${position.id}`}>
          <CardHeader>
            <h2 id={`position-${position.id}`} className="text-base font-semibold">
              {position.name} {!position.active && <Badge variant="outline">Nonaktif</Badge>}
            </h2>
            <CardAction className="flex gap-2">
              {!position.hamletId && (
                <Button type="button" variant="outline" onClick={() => openPosition({ position })}>
                  Ubah nama
                </Button>
              )}
              {!position.hamletId && !position.headOfVillage && position.tenures.length === 0 && (
                <Button type="button" variant="outline" onClick={() => onDeletePosition(position)}>
                  Hapus
                </Button>
              )}
            </CardAction>
          </CardHeader>
          <CardContent className="grid gap-4">
            {position.tenures.length > 0 ? (
              <DataTable>
                <TableHeader>
                  <TableRow>
                    <TableHead>Nama</TableHead>
                    <TableHead>Jenis</TableHead>
                    <TableHead>Masa jabatan</TableHead>
                    <TableHead>SK</TableHead>
                    <TableHead>Tindakan</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {position.tenures.map((item) => (
                    <TableRow key={item.id}>
                      <TableCell>
                        <span className="font-medium">{item.name}</span>
                        {item.nipd && (
                          <span className="block text-muted-foreground">NIPD {item.nipd}</span>
                        )}
                      </TableCell>
                      <TableCell>{tenureKindLabels[item.kind]}</TableCell>
                      <TableCell>{period(item)}</TableCell>
                      <TableCell>
                        {item.decreeNumber}
                        <span className="block text-muted-foreground">
                          {formatDate(item.decreeDate)}
                        </span>
                      </TableCell>
                      <TableCell>
                        <Button
                          type="button"
                          variant="outline"
                          onClick={() => openTenure({ position, tenure: item })}
                        >
                          Ubah
                        </Button>
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </DataTable>
            ) : (
              <p className="text-muted-foreground">Belum ada masa jabatan.</p>
            )}
            {position.active && (
              <Button
                type="button"
                variant="outline"
                className="w-fit"
                onClick={() => openTenure({ position })}
              >
                <Plus aria-hidden="true" />
                Tambah masa jabatan
              </Button>
            )}
          </CardContent>
        </Card>
      ))}
      <Button type="button" size="lg" className="w-fit px-4" onClick={() => openPosition({})}>
        <Plus aria-hidden="true" />
        Tambah jabatan
      </Button>
      <Dialog
        open={editingPosition !== null}
        onOpenChange={(isOpen) => !isOpen && setEditingPosition(null)}
      >
        <DialogContent>
          {editingPosition && (
            <form onSubmit={onSavePosition} className="grid gap-4">
              <DialogHeader>
                <DialogTitle>
                  {editingPosition.position
                    ? `Ubah nama ${editingPosition.position.name}`
                    : 'Tambah jabatan'}
                </DialogTitle>
              </DialogHeader>
              <div className="grid gap-2">
                <Label htmlFor="position-name">Nama jabatan</Label>
                <Input
                  id="position-name"
                  name="name"
                  required
                  maxLength={100}
                  defaultValue={editingPosition.position?.name ?? ''}
                />
              </div>
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
      <Dialog
        open={editingTenure !== null}
        onOpenChange={(isOpen) => !isOpen && setEditingTenure(null)}
      >
        <DialogContent className="max-h-[90dvh] overflow-y-auto">
          {editingTenure && (
            <form onSubmit={onSaveTenure} className="grid gap-4">
              <DialogHeader>
                <DialogTitle>
                  {tenure ? 'Ubah masa jabatan' : 'Tambah masa jabatan'} ·{' '}
                  {editingTenure.position.name}
                </DialogTitle>
              </DialogHeader>
              {!tenure && (
                <div className="grid gap-2">
                  <Label htmlFor="tenure-official">Pejabat</Label>
                  <NativeSelect
                    id="tenure-official"
                    value={officialId}
                    onChange={(event) => setOfficialId(event.currentTarget.value)}
                    className="w-full"
                  >
                    <NativeSelectOption value="">Orang baru</NativeSelectOption>
                    {officials.map((official) => (
                      <NativeSelectOption key={official.officialId} value={official.officialId}>
                        {official.name}
                      </NativeSelectOption>
                    ))}
                  </NativeSelect>
                </div>
              )}
              {/* Remounted per Official so the defaults follow the choice. */}
              <div key={officialId} className="grid gap-4 sm:grid-cols-2">
                <div className="grid gap-2">
                  <Label htmlFor="tenure-name">Nama</Label>
                  <Input
                    id="tenure-name"
                    name="name"
                    required
                    maxLength={100}
                    defaultValue={prefill?.name ?? ''}
                  />
                </div>
                <div className="grid gap-2">
                  <Label htmlFor="tenure-nipd">NIPD (opsional)</Label>
                  <Input
                    id="tenure-nipd"
                    name="nipd"
                    maxLength={30}
                    defaultValue={prefill?.nipd ?? ''}
                  />
                </div>
              </div>
              {officialId && !tenure && (
                <p className="text-muted-foreground">
                  Nama dan NIPD yang diubah berlaku untuk semua masa jabatan orang ini.
                </p>
              )}
              <div className="grid gap-4 sm:grid-cols-2">
                <div className="grid gap-2">
                  <Label htmlFor="tenure-kind">Jenis</Label>
                  <NativeSelect
                    id="tenure-kind"
                    name="kind"
                    defaultValue={tenure?.kind ?? 'definitive'}
                    className="w-full"
                  >
                    {(Object.keys(tenureKindLabels) as TenureKind[]).map((kind) => (
                      <NativeSelectOption key={kind} value={kind}>
                        {tenureKindLabels[kind]}
                      </NativeSelectOption>
                    ))}
                  </NativeSelect>
                </div>
                <div className="hidden sm:block" />
                <div className="grid gap-2">
                  <Label htmlFor="tenure-start">Tanggal mulai</Label>
                  <Input
                    id="tenure-start"
                    name="startDate"
                    type="date"
                    required
                    defaultValue={tenure?.startDate ?? ''}
                  />
                </div>
                <div className="grid gap-2">
                  <Label htmlFor="tenure-end">Tanggal selesai (opsional)</Label>
                  <Input
                    id="tenure-end"
                    name="endDate"
                    type="date"
                    defaultValue={tenure?.endDate ?? ''}
                  />
                </div>
                <div className="grid gap-2">
                  <Label htmlFor="tenure-decree-number">Nomor SK</Label>
                  <Input
                    id="tenure-decree-number"
                    name="decreeNumber"
                    required
                    maxLength={100}
                    defaultValue={tenure?.decreeNumber ?? ''}
                  />
                </div>
                <div className="grid gap-2">
                  <Label htmlFor="tenure-decree-date">Tanggal SK</Label>
                  <Input
                    id="tenure-decree-date"
                    name="decreeDate"
                    type="date"
                    required
                    defaultValue={tenure?.decreeDate ?? ''}
                  />
                </div>
              </div>
              <p className="text-muted-foreground">
                Hanya satu pejabat Definitif atau Pj. pada satu waktu; Plt. boleh bersamaan.
              </p>
              <div className="flex flex-wrap items-center gap-4">
                {photo ? (
                  <img
                    src={`/media/${photo}/480`}
                    alt=""
                    width={80}
                    height={80}
                    className="size-20 rounded-lg border object-cover"
                  />
                ) : (
                  <span className="grid size-20 place-items-center rounded-lg border text-muted-foreground">
                    Tanpa foto
                  </span>
                )}
                <div className="grid gap-2">
                  <Label htmlFor="tenure-photo">Foto (opsional)</Label>
                  <Input
                    id="tenure-photo"
                    type="file"
                    accept="image/jpeg,image/png,image/webp"
                    onChange={onPhoto}
                    disabled={uploading}
                  />
                </div>
                {photo && (
                  <Button type="button" variant="outline" onClick={() => setPhoto(null)}>
                    Hapus foto
                  </Button>
                )}
              </div>
              {photo && (
                <div className="grid gap-2">
                  <Label htmlFor="tenure-photo-alt">Teks alternatif foto</Label>
                  <Input
                    id="tenure-photo-alt"
                    name="photoAlt"
                    required
                    maxLength={200}
                    aria-describedby="tenure-photo-alt-hint"
                    defaultValue={tenure?.photoAlt ?? ''}
                  />
                  <p id="tenure-photo-alt-hint" className="text-muted-foreground">
                    Gambarkan foto untuk pembaca layar, misalnya "Budi Santoso berseragam dinas".
                  </p>
                </div>
              )}
              {dialogError && <Notice error>{dialogError}</Notice>}
              <DialogFooter>
                <DialogClose
                  render={<Button type="button" variant="outline" size="lg" className="px-4" />}
                >
                  Batal
                </DialogClose>
                <Button type="submit" size="lg" className="px-4" disabled={uploading}>
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
