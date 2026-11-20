import { createFileRoute, useRouter } from '@tanstack/react-router';
import { createServerFn, useServerFn } from '@tanstack/react-start';
import { ArrowDown, ArrowUp, Plus, X } from 'lucide-react';
import { type ChangeEvent, type FormEvent, useState } from 'react';
import {
  createInstitution,
  deleteInstitution,
  type Institution,
  type InstitutionFields,
  listInstitutions,
  type Member,
  updateInstitution,
  uploadVillageLogo,
} from '../../api/generated';
import { Notice, PageHead } from '../../components/admin';
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
import { Textarea } from '../../components/ui/textarea';
import { institutionTitle } from '../../institution';

// Server-only; the Institutions, or null when the API cannot be reached.
const fetchInstitutions = createServerFn({ method: 'GET' }).handler(async () => {
  const response = await listInstitutions();
  return response.status === 200 ? response.data : null;
});

const createInstitutionFn = createServerFn({ method: 'POST' })
  .validator((institution: InstitutionFields) => institution)
  .handler(async ({ data }) => (await createInstitution(data)).status);

const updateInstitutionFn = createServerFn({ method: 'POST' })
  .validator((data: { id: string; institution: InstitutionFields }) => data)
  .handler(async ({ data }) => (await updateInstitution(data.id, data.institution)).status);

const deleteInstitutionFn = createServerFn({ method: 'POST' })
  .validator((id: string) => id)
  .handler(async ({ data }) => (await deleteInstitution(data)).status);

const uploadLogoFn = createServerFn({ method: 'POST' })
  .validator((data: FormData) => {
    const logo = data.get('logo');
    if (!(logo instanceof File)) throw new Error('Expected a logo file');
    return logo;
  })
  .handler(async ({ data }) => {
    const response = await uploadVillageLogo(data);
    return response.status === 201
      ? { status: response.status, id: response.data.id }
      : { status: response.status };
  });

export const Route = createFileRoute('/admin/lembaga-desa')({
  loader: () => fetchInstitutions(),
  head: () => ({ meta: [{ title: 'Lembaga desa · Pranata' }] }),
  component: Institutions,
});

const MAX_LOGO_BYTES = 2 << 20;

const forbidden = 'Anda tidak memiliki izin mengelola data desa.';

const saveErrors: Record<number, string> = {
  403: forbidden,
  404: 'Lembaga ini sudah dihapus.',
  422: 'Periksa lagi isian: nama dan deskripsi lembaga wajib diisi, singkatan paling banyak 30 karakter, dan setiap pengurus wajib punya nama dan jabatan.',
};

const logoErrors: Record<number, string> = {
  403: forbidden,
  413: 'Logo paling besar 2 MB.',
  415: 'Logo harus berupa berkas PNG atau SVG.',
  422: 'Gambar rusak atau lebih dari 40 megapiksel.',
};

/** A member row being edited; the key keeps each row's inputs in place as rows move. */
type MemberRow = Member & { key: number };

let nextKey = 0;
const row = (member: Member = { name: '', role: '' }): MemberRow => ({
  ...member,
  key: nextKey++,
});

function Institutions() {
  const institutions = Route.useLoaderData();
  const router = useRouter();
  const saveNew = useServerFn(createInstitutionFn);
  const save = useServerFn(updateInstitutionFn);
  const remove = useServerFn(deleteInstitutionFn);
  const uploadLogo = useServerFn(uploadLogoFn);
  const [message, setMessage] = useState<{ text: string; error: boolean } | null>(null);
  // The Institution being edited, or null for a new one; undefined while the dialog is closed.
  const [editing, setEditing] = useState<Institution | null | undefined>(undefined);
  const [members, setMembers] = useState<MemberRow[]>([]);
  const [logo, setLogo] = useState<string | null>(null);
  const [uploading, setUploading] = useState(false);
  const [dialogError, setDialogError] = useState('');

  if (!institutions) {
    return (
      <>
        <PageHead title="Lembaga desa" />
        <Notice error>Gagal memuat lembaga.</Notice>
      </>
    );
  }

  const open = (institution: Institution | null) => {
    setDialogError('');
    setMembers((institution?.members ?? []).map((member) => row(member)));
    setLogo(institution?.logo ?? null);
    setEditing(institution);
  };

  const setMember = (index: number, field: keyof Member, value: string) =>
    setMembers(members.map((member, i) => (i === index ? { ...member, [field]: value } : member)));

  const move = (index: number, by: number) => {
    const next = [...members];
    const [member] = next.splice(index, 1);
    if (member) next.splice(index + by, 0, member);
    setMembers(next);
  };

  async function onLogo(event: ChangeEvent<HTMLInputElement>) {
    const file = event.currentTarget.files?.[0];
    event.currentTarget.value = '';
    if (!file) return;
    if (file.size > MAX_LOGO_BYTES) {
      setDialogError(logoErrors[413] ?? '');
      return;
    }
    setUploading(true);
    const data = new FormData();
    data.set('logo', file);
    const result = await uploadLogo({ data }).finally(() => setUploading(false));
    if (result.id) {
      setLogo(result.id);
      setDialogError('');
    } else {
      setDialogError(logoErrors[result.status] ?? 'Gagal mengunggah logo.');
    }
  }

  async function onSave(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (editing === undefined) return;
    const form = new FormData(event.currentTarget);
    const text = (name: string) => String(form.get(name) ?? '').trim() || null;
    const fields: InstitutionFields = {
      name: text('name') ?? '',
      abbreviation: text('abbreviation'),
      description: text('description') ?? '',
      logo,
      members: members.map(({ name, role }) => ({ name, role })),
    };
    const status = editing
      ? await save({ data: { id: editing.id, institution: fields } })
      : await saveNew({ data: fields });
    if (status >= 300) {
      setDialogError(saveErrors[status] ?? 'Gagal menyimpan.');
      return;
    }
    setEditing(undefined);
    setMessage({ text: 'Lembaga tersimpan.', error: false });
    await router.invalidate();
  }

  async function onDelete(institution: Institution) {
    if (!window.confirm(`Hapus ${institutionTitle(institution)} beserta pengurusnya?`)) return;
    const status = await remove({ data: institution.id });
    if (status >= 300) {
      setMessage({ text: saveErrors[status] ?? 'Gagal menghapus.', error: true });
      return;
    }
    setMessage({ text: 'Lembaga dihapus.', error: false });
    await router.invalidate();
  }

  return (
    <>
      <PageHead title="Lembaga desa">
        Badan di luar pemerintah desa, seperti BPD, LPM, PKK, dan BUMDes. Pengurusnya tidak
        menandatangani surat.
      </PageHead>
      {message && <Notice error={message.error}>{message.text}</Notice>}
      {institutions.length === 0 && <p className="text-muted-foreground">Belum ada lembaga.</p>}
      {institutions.map((institution) => (
        <Card key={institution.id} role="region" aria-labelledby={`institution-${institution.id}`}>
          <CardHeader>
            <h2 id={`institution-${institution.id}`} className="text-base font-semibold">
              {institutionTitle(institution)}
            </h2>
            <CardAction className="flex gap-2">
              <Button type="button" variant="outline" onClick={() => open(institution)}>
                Ubah
              </Button>
              <Button type="button" variant="outline" onClick={() => onDelete(institution)}>
                Hapus
              </Button>
            </CardAction>
          </CardHeader>
          <CardContent className="grid gap-3">
            <p>{institution.description}</p>
            {institution.members.length > 0 ? (
              <ol className="grid gap-1">
                {institution.members.map((member, index) => (
                  // Members have no id; their place in the list is who they are.
                  // biome-ignore lint/suspicious/noArrayIndexKey: see above
                  <li key={index}>
                    {member.name} <span className="text-muted-foreground">· {member.role}</span>
                  </li>
                ))}
              </ol>
            ) : (
              <p className="text-muted-foreground">Belum ada pengurus.</p>
            )}
          </CardContent>
        </Card>
      ))}
      <Button type="button" size="lg" className="w-fit px-4" onClick={() => open(null)}>
        <Plus aria-hidden="true" />
        Tambah lembaga
      </Button>
      <Dialog
        open={editing !== undefined}
        onOpenChange={(isOpen) => !isOpen && setEditing(undefined)}
      >
        <DialogContent className="max-h-[90dvh] overflow-y-auto sm:max-w-2xl">
          {editing !== undefined && (
            <form onSubmit={onSave} className="grid gap-4">
              <DialogHeader>
                <DialogTitle>
                  {editing ? `Ubah ${institutionTitle(editing)}` : 'Tambah lembaga'}
                </DialogTitle>
              </DialogHeader>
              <div className="grid gap-4 sm:grid-cols-[1fr_10rem]">
                <div className="grid gap-2">
                  <Label htmlFor="institution-name">Nama lembaga</Label>
                  <Input
                    id="institution-name"
                    name="name"
                    required
                    maxLength={100}
                    defaultValue={editing?.name ?? ''}
                  />
                </div>
                <div className="grid gap-2">
                  <Label htmlFor="institution-abbreviation">Singkatan (opsional)</Label>
                  <Input
                    id="institution-abbreviation"
                    name="abbreviation"
                    maxLength={30}
                    defaultValue={editing?.abbreviation ?? ''}
                  />
                </div>
              </div>
              <div className="grid gap-2">
                <Label htmlFor="institution-description">Deskripsi</Label>
                <Textarea
                  id="institution-description"
                  name="description"
                  required
                  maxLength={2000}
                  rows={3}
                  defaultValue={editing?.description ?? ''}
                />
              </div>
              <div className="flex flex-wrap items-center gap-4">
                {logo ? (
                  <img
                    src={`/media/${logo}/480`}
                    alt=""
                    width={80}
                    height={80}
                    className="size-20 rounded-lg border object-contain"
                  />
                ) : (
                  <span className="grid size-20 place-items-center rounded-lg border text-muted-foreground">
                    Tanpa logo
                  </span>
                )}
                <div className="grid gap-2">
                  <Label htmlFor="institution-logo">Logo (opsional)</Label>
                  <Input
                    id="institution-logo"
                    type="file"
                    accept="image/png,image/svg+xml"
                    onChange={onLogo}
                    disabled={uploading}
                  />
                </div>
                {logo && (
                  <Button type="button" variant="outline" onClick={() => setLogo(null)}>
                    Hapus logo
                  </Button>
                )}
              </div>
              <fieldset className="grid gap-2">
                <legend className="mb-2 text-sm font-medium">Pengurus/anggota, berurutan</legend>
                {members.map((member, index) => (
                  <div key={member.key} className="flex flex-wrap items-center gap-2">
                    <Input
                      aria-label={`Nama pengurus ${index + 1}`}
                      placeholder="Nama"
                      required
                      maxLength={100}
                      value={member.name}
                      onChange={(event) => setMember(index, 'name', event.currentTarget.value)}
                      className="min-w-40 flex-1"
                    />
                    <Input
                      aria-label={`Jabatan pengurus ${index + 1}`}
                      placeholder="Jabatan, mis. Ketua"
                      required
                      maxLength={100}
                      value={member.role}
                      onChange={(event) => setMember(index, 'role', event.currentTarget.value)}
                      className="min-w-32 flex-1"
                    />
                    <Button
                      type="button"
                      variant="outline"
                      size="icon"
                      aria-label={`Naikkan pengurus ${index + 1}`}
                      disabled={index === 0}
                      onClick={() => move(index, -1)}
                    >
                      <ArrowUp aria-hidden="true" />
                    </Button>
                    <Button
                      type="button"
                      variant="outline"
                      size="icon"
                      aria-label={`Turunkan pengurus ${index + 1}`}
                      disabled={index === members.length - 1}
                      onClick={() => move(index, 1)}
                    >
                      <ArrowDown aria-hidden="true" />
                    </Button>
                    <Button
                      type="button"
                      variant="outline"
                      size="icon"
                      aria-label={`Hapus pengurus ${index + 1}`}
                      onClick={() => setMembers(members.filter((_, i) => i !== index))}
                    >
                      <X aria-hidden="true" />
                    </Button>
                  </div>
                ))}
                <Button
                  type="button"
                  variant="outline"
                  className="w-fit"
                  disabled={members.length >= 100}
                  onClick={() => setMembers([...members, row()])}
                >
                  <Plus aria-hidden="true" />
                  Tambah pengurus
                </Button>
              </fieldset>
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
