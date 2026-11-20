import { createFileRoute, useRouter } from '@tanstack/react-router';
import { createServerFn, useServerFn } from '@tanstack/react-start';
import { FileText, Plus } from 'lucide-react';
import { type ChangeEvent, type FormEvent, useState } from 'react';
import {
  createTransparencyDocument,
  deleteTransparencyDocument,
  listTransparencyDocuments,
  publishTransparencyDocument,
  type TransparencyDocument,
  type TransparencyDocumentFields,
  type TransparencyDocumentType,
  unpublishTransparencyDocument,
  updateTransparencyDocument,
  uploadTransparencyDocumentFile,
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
import { TableBody, TableCell, TableHead, TableHeader, TableRow } from '../../components/ui/table';
import { documentTypeLabels, fileSize } from '../../transparency-document';

// Server-only; null without a content Permission.
const fetchDocuments = createServerFn({ method: 'GET' }).handler(async () => {
  const response = await listTransparencyDocuments();
  return {
    documents: response.status === 200 ? response.data : null,
    forbidden: response.status === 403,
  };
});

const saveDocumentFn = createServerFn({ method: 'POST' })
  .validator((data: { id: string | null; document: TransparencyDocumentFields }) => data)
  .handler(
    async ({ data }) =>
      (data.id
        ? await updateTransparencyDocument(data.id, data.document)
        : await createTransparencyDocument(data.document)
      ).status,
  );

const publishFn = createServerFn({ method: 'POST' })
  .validator((data: { id: string; publish: boolean }) => data)
  .handler(
    async ({ data }) =>
      (
        await (data.publish
          ? publishTransparencyDocument(data.id)
          : unpublishTransparencyDocument(data.id))
      ).status,
  );

const deleteDocumentFn = createServerFn({ method: 'POST' })
  .validator((id: string) => id)
  .handler(async ({ data }) => (await deleteTransparencyDocument(data)).status);

const uploadFileFn = createServerFn({ method: 'POST' })
  .validator((data: FormData) => {
    const file = data.get('file');
    if (!(file instanceof File)) throw new Error('Expected a PDF file');
    return file;
  })
  .handler(async ({ data }) => {
    const response = await uploadTransparencyDocumentFile(data);
    return response.status === 201
      ? { status: response.status, id: response.data.id }
      : { status: response.status };
  });

export const Route = createFileRoute('/admin/dokumen')({
  loader: () => fetchDocuments(),
  head: () => ({ meta: [{ title: 'Dokumen transparansi · Pranata' }] }),
  component: Documents,
});

const MAX_PDF_BYTES = 25 << 20;

const documentErrors: Record<number, string> = {
  403: 'Hanya pengguna dengan izin menerbitkan konten yang dapat melakukannya; dokumen yang sudah terbit hanya dapat diubah oleh penerbit.',
  404: 'Dokumen ini sudah dihapus.',
  409: 'Status dokumen sudah berubah; muat ulang.',
  422: 'Periksa lagi isian: judul wajib diisi (paling banyak 200 karakter), tahun anggaran antara 2000 dan 2100, dan berkas PDF wajib diunggah.',
};

const fileErrors: Record<number, string> = {
  403: 'Anda tidak memiliki izin menulis konten.',
  413: 'Berkas PDF paling besar 25 MB.',
  415: 'Berkas harus berupa PDF.',
};

function Documents() {
  const { documents, forbidden } = Route.useLoaderData();
  const router = useRouter();
  const saveDocument = useServerFn(saveDocumentFn);
  const setPublished = useServerFn(publishFn);
  const removeDocument = useServerFn(deleteDocumentFn);
  const uploadFile = useServerFn(uploadFileFn);
  const [message, setMessage] = useState<{ text: string; error: boolean } | null>(null);
  // The document being edited, or null for a new one; undefined while the dialog is closed.
  const [editing, setEditing] = useState<TransparencyDocument | null | undefined>(undefined);
  const [file, setFile] = useState<{ id: string; byteSize: number } | null>(null);
  const [uploading, setUploading] = useState(false);
  const [dialogError, setDialogError] = useState('');

  if (!documents) {
    return (
      <>
        <PageHead title="Dokumen transparansi" />
        <Notice error>
          {forbidden ? 'Anda tidak memiliki izin menulis konten.' : 'Gagal memuat dokumen.'}
        </Notice>
      </>
    );
  }

  async function result(status: number, done: string) {
    if (status >= 300) {
      setMessage({ text: documentErrors[status] ?? 'Gagal menyimpan.', error: true });
      return;
    }
    setMessage({ text: done, error: false });
    await router.invalidate();
  }

  const open = (document: TransparencyDocument | null) => {
    setDialogError('');
    setFile(document && { id: document.file, byteSize: document.byteSize });
    setEditing(document);
  };

  async function onFile(event: ChangeEvent<HTMLInputElement>) {
    const picked = event.currentTarget.files?.[0];
    event.currentTarget.value = '';
    if (!picked) return;
    if (picked.size > MAX_PDF_BYTES) {
      setDialogError(fileErrors[413] ?? '');
      return;
    }
    setUploading(true);
    const data = new FormData();
    data.set('file', picked);
    const uploaded = await uploadFile({ data }).finally(() => setUploading(false));
    if (uploaded.id) {
      setFile({ id: uploaded.id, byteSize: picked.size });
      setDialogError('');
    } else {
      setDialogError(fileErrors[uploaded.status] ?? 'Gagal mengunggah berkas.');
    }
  }

  async function onSave(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (editing === undefined) return;
    if (!file) {
      setDialogError('Unggah berkas PDF dulu.');
      return;
    }
    const form = new FormData(event.currentTarget);
    const document: TransparencyDocumentFields = {
      title: String(form.get('title') ?? '').trim(),
      documentType: form.get('documentType') as TransparencyDocumentType,
      fiscalYear: Number(form.get('fiscalYear')),
      file: file.id,
    };
    const status = await saveDocument({ data: { id: editing?.id ?? null, document } });
    if (status >= 300) {
      setDialogError(documentErrors[status] ?? 'Gagal menyimpan.');
      return;
    }
    setEditing(undefined);
    setMessage({ text: 'Dokumen tersimpan.', error: false });
    await router.invalidate();
  }

  async function onDelete(document: TransparencyDocument) {
    if (!window.confirm(`Hapus draf dokumen ${document.title}?`)) return;
    await result(await removeDocument({ data: document.id }), 'Dokumen dihapus.');
  }

  return (
    <>
      <PageHead title="Dokumen transparansi">
        Dokumen anggaran, perencanaan, dan peraturan desa tampil di halaman Dokumen situs setelah
        diterbitkan, untuk diunduh siapa saja.
      </PageHead>
      {message && <Notice error={message.error}>{message.text}</Notice>}
      <Button type="button" size="lg" className="w-fit px-4" onClick={() => open(null)}>
        <Plus aria-hidden="true" />
        Unggah dokumen
      </Button>
      {documents.length === 0 ? (
        <p className="text-muted-foreground">Belum ada dokumen.</p>
      ) : (
        <DataTable>
          <TableHeader>
            <TableRow>
              <TableHead>Judul</TableHead>
              <TableHead>Jenis</TableHead>
              <TableHead>Tahun anggaran</TableHead>
              <TableHead>Berkas</TableHead>
              <TableHead>Status</TableHead>
              <TableHead>
                <span className="sr-only">Tindakan</span>
              </TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {documents.map((document) => (
              <TableRow key={document.id}>
                <TableCell className="max-w-80 font-medium whitespace-normal">
                  {document.title}
                </TableCell>
                <TableCell>
                  <Badge variant="outline">{documentTypeLabels[document.documentType]}</Badge>
                </TableCell>
                <TableCell>{document.fiscalYear}</TableCell>
                <TableCell>
                  <a
                    href={`/media/${document.file}/pdf`}
                    download={`${document.title}.pdf`}
                    className="inline-flex items-center gap-1 text-muted-foreground hover:text-primary"
                  >
                    <FileText className="size-4" aria-hidden="true" />
                    {fileSize(document.byteSize)}
                  </a>
                </TableCell>
                <TableCell>
                  <Badge variant={document.published ? 'default' : 'secondary'}>
                    {document.published ? 'Terbit' : 'Draf'}
                  </Badge>
                </TableCell>
                <TableCell>
                  <div className="flex justify-end gap-2">
                    <Button type="button" variant="outline" onClick={() => open(document)}>
                      Ubah
                    </Button>
                    <Button
                      type="button"
                      variant="outline"
                      onClick={async () =>
                        result(
                          await setPublished({
                            data: { id: document.id, publish: !document.published },
                          }),
                          document.published
                            ? 'Dokumen kembali menjadi draf.'
                            : 'Dokumen terbit di halaman Dokumen.',
                        )
                      }
                    >
                      {document.published ? 'Batalkan terbit' : 'Terbitkan'}
                    </Button>
                    {!document.published && (
                      <Button type="button" variant="outline" onClick={() => onDelete(document)}>
                        Hapus
                      </Button>
                    )}
                  </div>
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </DataTable>
      )}
      <Dialog
        open={editing !== undefined}
        onOpenChange={(isOpen) => !isOpen && setEditing(undefined)}
      >
        <DialogContent>
          {editing !== undefined && (
            <form onSubmit={onSave} className="grid gap-4">
              <DialogHeader>
                <DialogTitle>{editing ? 'Ubah dokumen' : 'Unggah dokumen'}</DialogTitle>
              </DialogHeader>
              <div className="grid gap-2">
                <Label htmlFor="document-title">Judul</Label>
                <Input
                  id="document-title"
                  name="title"
                  required
                  maxLength={200}
                  defaultValue={editing?.title ?? ''}
                />
              </div>
              <div className="flex flex-wrap gap-4">
                <div className="grid gap-2">
                  <Label htmlFor="document-type">Jenis</Label>
                  <NativeSelect
                    id="document-type"
                    name="documentType"
                    defaultValue={editing?.documentType ?? 'apbdes'}
                  >
                    {(Object.keys(documentTypeLabels) as TransparencyDocumentType[]).map((type) => (
                      <NativeSelectOption key={type} value={type}>
                        {documentTypeLabels[type]}
                      </NativeSelectOption>
                    ))}
                  </NativeSelect>
                </div>
                <div className="grid gap-2">
                  <Label htmlFor="document-year">Tahun anggaran</Label>
                  <Input
                    id="document-year"
                    name="fiscalYear"
                    type="number"
                    required
                    min={2000}
                    max={2100}
                    className="w-28"
                    defaultValue={editing?.fiscalYear ?? new Date().getFullYear()}
                  />
                </div>
              </div>
              <div className="grid gap-2">
                <Label htmlFor="document-file">Berkas PDF (paling besar 25 MB)</Label>
                <Input
                  id="document-file"
                  type="file"
                  accept="application/pdf"
                  onChange={onFile}
                  disabled={uploading}
                />
                <p className="text-sm text-muted-foreground">
                  {uploading
                    ? 'Mengunggah…'
                    : file
                      ? `Berkas terunggah · PDF · ${fileSize(file.byteSize)}`
                      : 'Belum ada berkas.'}
                </p>
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
