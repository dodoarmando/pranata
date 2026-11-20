import { createFileRoute, useRouter } from '@tanstack/react-router';
import { createServerFn, useServerFn } from '@tanstack/react-start';
import { ArrowDown, ArrowUp, Plus } from 'lucide-react';
import { type FormEvent, useState } from 'react';
import {
  type Category,
  createContactCategory,
  createImportantContact,
  deleteContactCategory,
  deleteImportantContact,
  type ImportantContact,
  type ImportantContactFields,
  listContactCategories,
  listImportantContacts,
  orderContactCategories,
  publishImportantContact,
  unpublishImportantContact,
  updateContactCategory,
  updateImportantContact,
} from '../../api/generated';
import { DataTable, Notice, PageHead } from '../../components/admin';
import { Badge } from '../../components/ui/badge';
import { Button } from '../../components/ui/button';
import { Card, CardContent, CardHeader } from '../../components/ui/card';
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

// Server-only; the contacts are null without a content Permission.
const fetchContacts = createServerFn({ method: 'GET' }).handler(async () => {
  const [contacts, categories] = await Promise.all([
    listImportantContacts(),
    listContactCategories(),
  ]);
  return {
    contacts: contacts.status === 200 ? contacts.data : null,
    forbidden: contacts.status === 403,
    categories: categories.status === 200 ? categories.data : [],
  };
});

const saveContactFn = createServerFn({ method: 'POST' })
  .validator((data: { id: string | null; contact: ImportantContactFields }) => data)
  .handler(
    async ({ data }) =>
      (data.id
        ? await updateImportantContact(data.id, data.contact)
        : await createImportantContact(data.contact)
      ).status,
  );

const publishFn = createServerFn({ method: 'POST' })
  .validator((data: { id: string; publish: boolean }) => data)
  .handler(
    async ({ data }) =>
      (await (data.publish ? publishImportantContact(data.id) : unpublishImportantContact(data.id)))
        .status,
  );

const deleteContactFn = createServerFn({ method: 'POST' })
  .validator((id: string) => id)
  .handler(async ({ data }) => (await deleteImportantContact(data)).status);

const createCategoryFn = createServerFn({ method: 'POST' })
  .validator((name: string) => name)
  .handler(async ({ data }) => (await createContactCategory({ name: data })).status);

const renameCategoryFn = createServerFn({ method: 'POST' })
  .validator((data: { id: string; name: string }) => data)
  .handler(async ({ data }) => (await updateContactCategory(data.id, { name: data.name })).status);

const deleteCategoryFn = createServerFn({ method: 'POST' })
  .validator((id: string) => id)
  .handler(async ({ data }) => (await deleteContactCategory(data)).status);

const orderCategoriesFn = createServerFn({ method: 'POST' })
  .validator((ids: string[]) => ids)
  .handler(async ({ data }) => (await orderContactCategories({ ids: data })).status);

export const Route = createFileRoute('/admin/kontak-penting')({
  loader: () => fetchContacts(),
  head: () => ({ meta: [{ title: 'Kontak penting · Pranata' }] }),
  component: Contacts,
});

const publisherOnly =
  'Hanya pengguna dengan izin menerbitkan konten yang dapat melakukannya; kontak yang sudah terbit hanya dapat diubah oleh penerbit.';

const contactErrors: Record<number, string> = {
  403: publisherOnly,
  404: 'Kontak ini sudah dihapus.',
  409: 'Status kontak sudah berubah; muat ulang.',
  422: 'Periksa lagi isian: nama wajib diisi (paling banyak 100 karakter), kategori wajib dipilih, nomor telepon minimal 3 angka (hanya angka, spasi, dan + - ( )), dan keterangan paling banyak 150 karakter.',
};

const categoryErrors: Record<number, string> = {
  403: 'Hanya pengguna dengan izin menerbitkan konten yang dapat mengelola kategori kontak.',
  404: 'Kategori ini sudah dihapus.',
  409: 'Nama kategori sudah dipakai, atau kategori masih dipakai kontak.',
  422: 'Nama kategori wajib diisi, paling banyak 50 karakter. Bila mengurutkan, muat ulang dulu.',
};

function Contacts() {
  const { contacts, forbidden, categories } = Route.useLoaderData();
  const router = useRouter();
  const saveContact = useServerFn(saveContactFn);
  const setPublished = useServerFn(publishFn);
  const removeContact = useServerFn(deleteContactFn);
  const addCategory = useServerFn(createCategoryFn);
  const renameCategory = useServerFn(renameCategoryFn);
  const removeCategory = useServerFn(deleteCategoryFn);
  const orderCategories = useServerFn(orderCategoriesFn);
  const [message, setMessage] = useState<{ text: string; error: boolean } | null>(null);
  // The contact being edited, or null for a new one; undefined while the dialog is closed.
  const [editing, setEditing] = useState<ImportantContact | null | undefined>(undefined);
  const [dialogError, setDialogError] = useState('');

  if (!contacts) {
    return (
      <>
        <PageHead title="Kontak penting" />
        <Notice error>
          {forbidden ? 'Anda tidak memiliki izin menulis konten.' : 'Gagal memuat kontak.'}
        </Notice>
      </>
    );
  }

  async function result(status: number, done: string, errors: Record<number, string>) {
    if (status >= 300) {
      setMessage({ text: errors[status] ?? 'Gagal menyimpan.', error: true });
      return false;
    }
    setMessage({ text: done, error: false });
    await router.invalidate();
    return true;
  }

  async function onSave(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (editing === undefined) return;
    const form = new FormData(event.currentTarget);
    const text = (name: string) => String(form.get(name) ?? '').trim();
    const order = text('order');
    const contact: ImportantContactFields = {
      name: text('name'),
      category: text('category'),
      phone: text('phone'),
      note: text('note') || null,
      order: order ? Number(order) : null,
    };
    const status = await saveContact({ data: { id: editing?.id ?? null, contact } });
    if (status >= 300) {
      setDialogError(contactErrors[status] ?? 'Gagal menyimpan.');
      return;
    }
    setEditing(undefined);
    setMessage({ text: 'Kontak tersimpan.', error: false });
    await router.invalidate();
  }

  async function onPublish(contact: ImportantContact, publish: boolean) {
    await result(
      await setPublished({ data: { id: contact.id, publish } }),
      publish ? 'Kontak terbit di situs.' : 'Kontak kembali menjadi draf.',
      contactErrors,
    );
  }

  async function onDelete(contact: ImportantContact) {
    if (!window.confirm(`Hapus draf kontak ${contact.name}?`)) return;
    await result(await removeContact({ data: contact.id }), 'Kontak dihapus.', contactErrors);
  }

  async function onAddCategory(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const form = event.currentTarget;
    const name = String(new FormData(form).get('name') ?? '').trim();
    if (await result(await addCategory({ data: name }), 'Kategori ditambahkan.', categoryErrors)) {
      form.reset();
    }
  }

  async function onRename(category: Category) {
    const name = window.prompt('Nama kategori', category.name)?.trim();
    if (!name || name === category.name) return;
    await result(
      await renameCategory({ data: { id: category.id, name } }),
      'Kategori diubah.',
      categoryErrors,
    );
  }

  async function onDeleteCategory(category: Category) {
    if (!window.confirm(`Hapus kategori ${category.name}?`)) return;
    await result(await removeCategory({ data: category.id }), 'Kategori dihapus.', categoryErrors);
  }

  async function onMoveCategory(index: number, by: number) {
    const ids = categories.map((category) => category.id);
    const [id] = ids.splice(index, 1);
    if (id) ids.splice(index + by, 0, id);
    await result(await orderCategories({ data: ids }), 'Urutan kategori diubah.', categoryErrors);
  }

  const open = (contact: ImportantContact | null) => {
    setDialogError('');
    setEditing(contact);
  };

  return (
    <>
      <PageHead title="Kontak penting">
        Nomor telepon yang mungkin dibutuhkan warga, dikelompokkan per kategori. Kontak tampil di
        halaman Kontak setelah diterbitkan.
      </PageHead>
      {message && <Notice error={message.error}>{message.text}</Notice>}
      <Button
        type="button"
        size="lg"
        className="w-fit px-4"
        disabled={categories.length === 0}
        onClick={() => open(null)}
      >
        <Plus aria-hidden="true" />
        Tambah kontak
      </Button>
      {categories.length === 0 && (
        <p className="text-muted-foreground">Tambahkan kategori kontak dulu.</p>
      )}
      {categories.map((category) => {
        const listed = contacts.filter((contact) => contact.category === category.id);
        return (
          listed.length > 0 && (
            <section
              key={category.id}
              aria-labelledby={`category-${category.id}`}
              className="grid gap-2"
            >
              <h2 id={`category-${category.id}`} className="text-base font-semibold">
                {category.name}
              </h2>
              <DataTable>
                <TableHeader>
                  <TableRow>
                    <TableHead>Urutan</TableHead>
                    <TableHead>Nama</TableHead>
                    <TableHead>Telepon</TableHead>
                    <TableHead>Keterangan</TableHead>
                    <TableHead>Status</TableHead>
                    <TableHead>
                      <span className="sr-only">Tindakan</span>
                    </TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {listed.map((contact) => (
                    <TableRow key={contact.id}>
                      <TableCell>{contact.order}</TableCell>
                      <TableCell className="font-medium">{contact.name}</TableCell>
                      <TableCell>{contact.phone}</TableCell>
                      <TableCell className="max-w-64 truncate">{contact.note ?? '—'}</TableCell>
                      <TableCell>
                        <Badge variant={contact.published ? 'default' : 'secondary'}>
                          {contact.published ? 'Terbit' : 'Draf'}
                        </Badge>
                      </TableCell>
                      <TableCell>
                        <div className="flex justify-end gap-2">
                          <Button type="button" variant="outline" onClick={() => open(contact)}>
                            Ubah
                          </Button>
                          <Button
                            type="button"
                            variant="outline"
                            onClick={() => onPublish(contact, !contact.published)}
                          >
                            {contact.published ? 'Batalkan terbit' : 'Terbitkan'}
                          </Button>
                          {!contact.published && (
                            <Button
                              type="button"
                              variant="outline"
                              onClick={() => onDelete(contact)}
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
            </section>
          )
        );
      })}
      <Card role="region" aria-labelledby="categories-title">
        <CardHeader>
          <h2 id="categories-title" className="text-base font-semibold">
            Kategori kontak
          </h2>
        </CardHeader>
        <CardContent className="grid gap-3">
          <ol className="grid gap-1">
            {categories.map((category, index) => (
              <li key={category.id} className="flex flex-wrap items-center gap-2">
                <span className="flex-1">{category.name}</span>
                <Button
                  type="button"
                  variant="outline"
                  size="icon"
                  aria-label={`Naikkan ${category.name}`}
                  disabled={index === 0}
                  onClick={() => onMoveCategory(index, -1)}
                >
                  <ArrowUp aria-hidden="true" />
                </Button>
                <Button
                  type="button"
                  variant="outline"
                  size="icon"
                  aria-label={`Turunkan ${category.name}`}
                  disabled={index === categories.length - 1}
                  onClick={() => onMoveCategory(index, 1)}
                >
                  <ArrowDown aria-hidden="true" />
                </Button>
                <Button type="button" variant="outline" onClick={() => onRename(category)}>
                  Ubah
                </Button>
                <Button type="button" variant="outline" onClick={() => onDeleteCategory(category)}>
                  Hapus
                </Button>
              </li>
            ))}
          </ol>
          <form onSubmit={onAddCategory} className="flex flex-wrap items-end gap-2">
            <div className="grid gap-2">
              <Label htmlFor="category-name">Kategori baru</Label>
              <Input id="category-name" name="name" required maxLength={50} />
            </div>
            <Button type="submit" variant="outline">
              Tambah kategori
            </Button>
          </form>
        </CardContent>
      </Card>
      <Dialog
        open={editing !== undefined}
        onOpenChange={(isOpen) => !isOpen && setEditing(undefined)}
      >
        <DialogContent>
          {editing !== undefined && (
            <form onSubmit={onSave} className="grid gap-4">
              <DialogHeader>
                <DialogTitle>{editing ? `Ubah ${editing.name}` : 'Tambah kontak'}</DialogTitle>
              </DialogHeader>
              <div className="grid gap-2">
                <Label htmlFor="contact-name">Nama</Label>
                <Input
                  id="contact-name"
                  name="name"
                  required
                  maxLength={100}
                  placeholder="mis. Puskesmas Kecamatan"
                  defaultValue={editing?.name ?? ''}
                />
              </div>
              <div className="grid gap-2">
                <Label htmlFor="contact-category">Kategori</Label>
                <NativeSelect
                  id="contact-category"
                  name="category"
                  required
                  className="w-full"
                  defaultValue={editing?.category ?? categories[0]?.id}
                >
                  {categories.map((category) => (
                    <NativeSelectOption key={category.id} value={category.id}>
                      {category.name}
                    </NativeSelectOption>
                  ))}
                </NativeSelect>
              </div>
              <div className="grid gap-2">
                <Label htmlFor="contact-phone">Telepon</Label>
                <Input
                  id="contact-phone"
                  name="phone"
                  type="tel"
                  required
                  maxLength={30}
                  defaultValue={editing?.phone ?? ''}
                />
              </div>
              <div className="grid gap-2">
                <Label htmlFor="contact-note">Keterangan (opsional)</Label>
                <Input
                  id="contact-note"
                  name="note"
                  maxLength={150}
                  placeholder="mis. Buka 24 jam"
                  defaultValue={editing?.note ?? ''}
                />
              </div>
              <div className="grid gap-2">
                <Label htmlFor="contact-order">Urutan dalam kategori (opsional)</Label>
                <Input
                  id="contact-order"
                  name="order"
                  type="number"
                  step={1}
                  className="w-32"
                  defaultValue={editing?.order ?? ''}
                />
                <p className="text-sm text-muted-foreground">
                  Angka kecil tampil lebih dulu. Kosongkan untuk menaruh kontak baru di akhir.
                </p>
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
    </>
  );
}
