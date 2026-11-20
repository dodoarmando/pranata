import { createFileRoute, useRouter } from '@tanstack/react-router';
import { createServerFn, useServerFn } from '@tanstack/react-start';
import { ArrowDown, ArrowUp, GripVertical, Plus } from 'lucide-react';
import { type ChangeEvent, type FormEvent, useState } from 'react';
import {
  createPartner,
  deletePartner,
  listPartners,
  orderPartners,
  type Partner,
  type PartnerFields,
  publishPartner,
  unpublishPartner,
  updatePartner,
  uploadPartnerLogo,
} from '../../api/generated';
import { Notice, PageHead } from '../../components/admin';
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

// Server-only; null without a content Permission.
const fetchPartners = createServerFn({ method: 'GET' }).handler(async () => {
  const response = await listPartners();
  return {
    partners: response.status === 200 ? response.data : null,
    forbidden: response.status === 403,
  };
});

const savePartnerFn = createServerFn({ method: 'POST' })
  .validator((data: { id: string | null; partner: PartnerFields }) => data)
  .handler(
    async ({ data }) =>
      (data.id ? await updatePartner(data.id, data.partner) : await createPartner(data.partner))
        .status,
  );

const publishFn = createServerFn({ method: 'POST' })
  .validator((data: { id: string; publish: boolean }) => data)
  .handler(
    async ({ data }) =>
      (await (data.publish ? publishPartner(data.id) : unpublishPartner(data.id))).status,
  );

const deletePartnerFn = createServerFn({ method: 'POST' })
  .validator((id: string) => id)
  .handler(async ({ data }) => (await deletePartner(data)).status);

const orderPartnersFn = createServerFn({ method: 'POST' })
  .validator((ids: string[]) => ids)
  .handler(async ({ data }) => (await orderPartners({ ids: data })).status);

const uploadLogoFn = createServerFn({ method: 'POST' })
  .validator((data: FormData) => {
    const logo = data.get('logo');
    if (!(logo instanceof File)) throw new Error('Expected a logo file');
    return logo;
  })
  .handler(async ({ data }) => {
    const response = await uploadPartnerLogo(data);
    return response.status === 201
      ? { status: response.status, id: response.data.id }
      : { status: response.status };
  });

export const Route = createFileRoute('/admin/mitra')({
  loader: () => fetchPartners(),
  head: () => ({ meta: [{ title: 'Mitra · Pranata' }] }),
  component: Partners,
});

const MAX_LOGO_BYTES = 2 << 20;

const partnerErrors: Record<number, string> = {
  403: 'Hanya pengguna dengan izin menerbitkan konten yang dapat melakukannya; mitra yang sudah terbit hanya dapat diubah oleh penerbit.',
  404: 'Mitra ini sudah dihapus.',
  409: 'Status mitra sudah berubah; muat ulang.',
  422: 'Periksa lagi isian: nama wajib diisi (paling banyak 100 karakter), logo wajib diunggah, dan alamat situs harus diawali http:// atau https://. Bila mengurutkan, muat ulang dulu.',
};

const logoErrors: Record<number, string> = {
  403: 'Anda tidak memiliki izin menulis konten.',
  413: 'Logo paling besar 2 MB.',
  415: 'Logo harus berupa berkas PNG.',
  422: 'Gambar rusak atau lebih dari 40 megapiksel.',
};

function Partners() {
  const { partners, forbidden } = Route.useLoaderData();
  const router = useRouter();
  const savePartner = useServerFn(savePartnerFn);
  const setPublished = useServerFn(publishFn);
  const removePartner = useServerFn(deletePartnerFn);
  const order = useServerFn(orderPartnersFn);
  const uploadLogo = useServerFn(uploadLogoFn);
  const [message, setMessage] = useState<{ text: string; error: boolean } | null>(null);
  // The Partner being edited, or null for a new one; undefined while the dialog is closed.
  const [editing, setEditing] = useState<Partner | null | undefined>(undefined);
  const [logo, setLogo] = useState<string | null>(null);
  const [uploading, setUploading] = useState(false);
  const [dialogError, setDialogError] = useState('');
  const [dragged, setDragged] = useState<number | null>(null);

  if (!partners) {
    return (
      <>
        <PageHead title="Mitra" />
        <Notice error>
          {forbidden ? 'Anda tidak memiliki izin menulis konten.' : 'Gagal memuat mitra.'}
        </Notice>
      </>
    );
  }

  async function result(status: number, done: string) {
    if (status >= 300) {
      setMessage({ text: partnerErrors[status] ?? 'Gagal menyimpan.', error: true });
      return;
    }
    setMessage({ text: done, error: false });
    await router.invalidate();
  }

  const open = (partner: Partner | null) => {
    setDialogError('');
    setLogo(partner?.logo ?? null);
    setEditing(partner);
  };

  async function move(from: number, to: number) {
    if (!partners || from === to) return;
    const ids = partners.map((partner) => partner.id);
    const [id] = ids.splice(from, 1);
    if (id) ids.splice(to, 0, id);
    await result(await order({ data: ids }), 'Urutan mitra diubah.');
  }

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
    const uploaded = await uploadLogo({ data }).finally(() => setUploading(false));
    if (uploaded.id) {
      setLogo(uploaded.id);
      setDialogError('');
    } else {
      setDialogError(logoErrors[uploaded.status] ?? 'Gagal mengunggah logo.');
    }
  }

  async function onSave(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (editing === undefined) return;
    if (!logo) {
      setDialogError('Unggah logo mitra dulu.');
      return;
    }
    const form = new FormData(event.currentTarget);
    const text = (name: string) => String(form.get(name) ?? '').trim();
    const partner: PartnerFields = { name: text('name'), logo, url: text('url') || null };
    const status = await savePartner({ data: { id: editing?.id ?? null, partner } });
    if (status >= 300) {
      setDialogError(partnerErrors[status] ?? 'Gagal menyimpan.');
      return;
    }
    setEditing(undefined);
    setMessage({ text: 'Mitra tersimpan.', error: false });
    await router.invalidate();
  }

  async function onDelete(partner: Partner) {
    if (!window.confirm(`Hapus draf mitra ${partner.name}?`)) return;
    await result(await removePartner({ data: partner.id }), 'Mitra dihapus.');
  }

  return (
    <>
      <PageHead title="Mitra">
        Logo mitra tampil di beranda situs setelah diterbitkan, sesuai urutan di sini. Seret baris
        atau pakai tombol panah untuk mengubah urutan.
      </PageHead>
      {message && <Notice error={message.error}>{message.text}</Notice>}
      <Button type="button" size="lg" className="w-fit px-4" onClick={() => open(null)}>
        <Plus aria-hidden="true" />
        Tambah mitra
      </Button>
      {partners.length === 0 ? (
        <p className="text-muted-foreground">Belum ada mitra.</p>
      ) : (
        <ol className="grid gap-2">
          {partners.map((partner, index) => (
            <li
              key={partner.id}
              draggable
              onDragStart={() => setDragged(index)}
              onDragEnd={() => setDragged(null)}
              onDragOver={(event) => event.preventDefault()}
              onDrop={() => dragged !== null && move(dragged, index)}
              className="flex flex-wrap items-center gap-3 rounded-lg border bg-card p-2 data-[dragged=true]:opacity-50"
              data-dragged={dragged === index}
            >
              <GripVertical
                className="size-4 flex-none cursor-grab text-muted-foreground"
                aria-hidden="true"
              />
              <img
                src={`/media/${partner.logo}/480`}
                alt=""
                width={96}
                height={48}
                className="h-12 w-24 object-contain"
              />
              <span className="grid min-w-40 flex-1">
                <span className="font-medium">{partner.name}</span>
                {partner.url && (
                  <span className="truncate text-sm text-muted-foreground">{partner.url}</span>
                )}
              </span>
              <Badge variant={partner.published ? 'default' : 'secondary'}>
                {partner.published ? 'Terbit' : 'Draf'}
              </Badge>
              <Button
                type="button"
                variant="outline"
                size="icon"
                aria-label={`Naikkan ${partner.name}`}
                disabled={index === 0}
                onClick={() => move(index, index - 1)}
              >
                <ArrowUp aria-hidden="true" />
              </Button>
              <Button
                type="button"
                variant="outline"
                size="icon"
                aria-label={`Turunkan ${partner.name}`}
                disabled={index === partners.length - 1}
                onClick={() => move(index, index + 1)}
              >
                <ArrowDown aria-hidden="true" />
              </Button>
              <Button type="button" variant="outline" onClick={() => open(partner)}>
                Ubah
              </Button>
              <Button
                type="button"
                variant="outline"
                onClick={async () =>
                  result(
                    await setPublished({
                      data: { id: partner.id, publish: !partner.published },
                    }),
                    partner.published ? 'Mitra kembali menjadi draf.' : 'Mitra terbit di beranda.',
                  )
                }
              >
                {partner.published ? 'Batalkan terbit' : 'Terbitkan'}
              </Button>
              {!partner.published && (
                <Button type="button" variant="outline" onClick={() => onDelete(partner)}>
                  Hapus
                </Button>
              )}
            </li>
          ))}
        </ol>
      )}
      <Dialog
        open={editing !== undefined}
        onOpenChange={(isOpen) => !isOpen && setEditing(undefined)}
      >
        <DialogContent>
          {editing !== undefined && (
            <form onSubmit={onSave} className="grid gap-4">
              <DialogHeader>
                <DialogTitle>{editing ? `Ubah ${editing.name}` : 'Tambah mitra'}</DialogTitle>
              </DialogHeader>
              <div className="grid gap-2">
                <Label htmlFor="partner-name">Nama</Label>
                <Input
                  id="partner-name"
                  name="name"
                  required
                  maxLength={100}
                  defaultValue={editing?.name ?? ''}
                />
              </div>
              <div className="flex flex-wrap items-center gap-4">
                {logo ? (
                  <img
                    src={`/media/${logo}/480`}
                    alt=""
                    width={96}
                    height={48}
                    className="h-12 w-24 rounded-lg border object-contain"
                  />
                ) : (
                  <span className="grid h-12 w-24 place-items-center rounded-lg border text-sm text-muted-foreground">
                    Tanpa logo
                  </span>
                )}
                <div className="grid gap-2">
                  <Label htmlFor="partner-logo">Logo (PNG, paling besar 2 MB)</Label>
                  <Input
                    id="partner-logo"
                    type="file"
                    accept="image/png"
                    onChange={onLogo}
                    disabled={uploading}
                  />
                </div>
              </div>
              <div className="grid gap-2">
                <Label htmlFor="partner-url">Alamat situs (opsional)</Label>
                <Input
                  id="partner-url"
                  name="url"
                  type="url"
                  maxLength={300}
                  placeholder="https://"
                  defaultValue={editing?.url ?? ''}
                />
              </div>
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
