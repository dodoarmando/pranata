import { createFileRoute, getRouteApi, Link } from '@tanstack/react-router';
import { createServerFn, useServerFn } from '@tanstack/react-start';
import { ChevronRight, ChevronsLeft, Download } from 'lucide-react';
import { type FormEvent, useState } from 'react';
import {
  type AuditEntry,
  type EncryptedFile,
  exportAuditEntries,
  type ListAuditEntriesParams,
  listAuditEntries,
} from '../../api/generated';
import { DataTable, Notice, PageHead } from '../../components/admin';
import { Badge } from '../../components/ui/badge';
import { Button, buttonVariants } from '../../components/ui/button';
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '../../components/ui/dialog';
import { Input } from '../../components/ui/input';
import { Label } from '../../components/ui/label';
import { TableBody, TableCell, TableHead, TableHeader, TableRow } from '../../components/ui/table';
import { useSudo } from '../../sudo';
import { ianaZones } from '../../village';

const filterKeys = ['objectType', 'objectId', 'username', 'from', 'to', 'before'] as const;

// Drops what a submitted filter form leaves empty; the router reads `objectId=7` as a number.
// Also the server function's validator, as the router merges the raw query back into the search.
const filterOf = (search: Record<string, unknown>): ListAuditEntriesParams =>
  Object.fromEntries(
    filterKeys.flatMap((key) => {
      const value = search[key];
      return value === undefined || value === '' ? [] : [[key, String(value)]];
    }),
  );

const shown = (value: unknown) =>
  value === null ? '(kosong)' : typeof value === 'string' ? value : JSON.stringify(value);

// Server-only; the page with its details and diff values as text, or the status that refused it.
const fetchAuditPage = createServerFn({ method: 'GET' })
  .validator(filterOf)
  .handler(async ({ data }) => {
    const response = await listAuditEntries(data);
    if (response.status !== 200) return response.status;
    const { entries, next } = response.data;
    return {
      next: next ?? null,
      entries: entries.map(({ details, changes, ...entry }) => ({
        ...entry,
        details: JSON.stringify(details) === '{}' ? '' : JSON.stringify(details),
        changes: changes.map(({ field, from, to }) => ({
          field,
          from: shown(from),
          to: shown(to),
        })),
      })),
    };
  });

// Server-only; the encrypted file, or the status that refused it.
const exportAuditFn = createServerFn({ method: 'POST' })
  .validator((data: { filter: ListAuditEntriesParams; reason: string }) => ({
    filter: filterOf(data.filter),
    reason: String(data.reason),
  }))
  .handler(async ({ data }) => {
    const response = await exportAuditEntries({ reason: data.reason }, data.filter);
    return response.status === 200 ? response.data : response.status;
  });

function download({ fileName, content }: EncryptedFile) {
  const bytes = Uint8Array.from(atob(content), (char) => char.charCodeAt(0));
  const url = URL.createObjectURL(new Blob([bytes], { type: 'application/zip' }));
  Object.assign(document.createElement('a'), { href: url, download: fileName }).click();
  setTimeout(() => URL.revokeObjectURL(url));
}

const root = getRouteApi('__root__');

export const Route = createFileRoute('/admin/log-audit')({
  validateSearch: filterOf,
  loaderDeps: ({ search }) => search,
  loader: ({ deps }) => fetchAuditPage({ data: deps }),
  head: () => ({ meta: [{ title: 'Log audit · Pranata' }] }),
  component: AuditLog,
});

const actor = (entry: Pick<AuditEntry, 'actor' | 'username'>) =>
  entry.username ?? (entry.actor === 'system' ? 'Sistem' : 'Tidak dikenal');

function AuditLog() {
  const village = root.useLoaderData();
  const page = Route.useLoaderData();
  const filter = Route.useSearch();
  const { before, ...newest } = filter;
  const zone = village?.timeZone ?? 'WIB';
  const time = new Intl.DateTimeFormat('id-ID', {
    dateStyle: 'medium',
    timeStyle: 'medium',
    timeZone: ianaZones[zone],
  });
  const sudo = useSudo();
  const submitExport = useServerFn(exportAuditFn);
  const [asking, setAsking] = useState(false);
  const [password, setPassword] = useState('');
  const [message, setMessage] = useState('');

  function onExport(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const reason = String(new FormData(event.currentTarget).get('reason'));
    // Closed first, so the password prompt of sudo mode is the only dialog open.
    setAsking(false);
    setMessage('');
    return sudo.run(async () => {
      const result = await submitExport({ data: { filter: newest, reason } });
      if (typeof result === 'number') {
        if (result !== 428) setMessage('Ekspor gagal. Coba lagi.');
        return result;
      }
      download(result);
      setPassword(result.password);
      return 200;
    });
  }

  return (
    <>
      <div className="flex flex-wrap items-start justify-between gap-2">
        <PageHead title="Log audit" />
        {page !== 403 && (
          <Button variant="outline" onClick={() => setAsking(true)}>
            <Download aria-hidden="true" />
            Ekspor CSV
          </Button>
        )}
      </div>
      {sudo.prompt}
      {message && <Notice error>{message}</Notice>}
      <Dialog open={asking} onOpenChange={setAsking}>
        <DialogContent>
          <form onSubmit={onExport} className="grid gap-4">
            <DialogHeader>
              <DialogTitle>Ekspor log audit</DialogTitle>
              <DialogDescription>
                Semua entri yang cocok dengan saringan saat ini diunduh sebagai CSV dalam berkas ZIP
                terenkripsi. Alasan ekspor dicatat di log audit.
              </DialogDescription>
            </DialogHeader>
            <div className="grid gap-2">
              <Label htmlFor="export-reason">Alasan ekspor</Label>
              <Input id="export-reason" name="reason" required pattern=".*\S.*" />
            </div>
            <DialogFooter>
              <DialogClose
                render={<Button type="button" variant="outline" size="lg" className="px-4" />}
              >
                Batal
              </DialogClose>
              <Button type="submit" size="lg" className="px-4">
                Ekspor
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
      <Dialog open={password !== ''} onOpenChange={(open) => open || setPassword('')}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Kata sandi berkas</DialogTitle>
            <DialogDescription>
              Berkas ZIP sedang diunduh. Bukalah dengan 7-Zip atau aplikasi lain yang mendukung
              enkripsi AES, memakai kata sandi di bawah. Catat sekarang: kata sandi ini hanya
              ditampilkan sekali.
            </DialogDescription>
          </DialogHeader>
          <output className="rounded-md bg-muted p-3 text-center font-mono text-lg select-all">
            {password}
          </output>
          <DialogFooter>
            <DialogClose render={<Button type="button" size="lg" className="px-4" />}>
              Selesai
            </DialogClose>
          </DialogFooter>
        </DialogContent>
      </Dialog>
      <form method="get" className="flex flex-wrap items-end gap-2">
        <Filter label="Jenis objek" name="objectType" value={filter.objectType} />
        <Filter label="ID objek" name="objectId" value={filter.objectId} />
        <Filter label="Pengguna" name="username" value={filter.username} />
        <Filter label="Dari tanggal" name="from" type="date" value={filter.from} />
        <Filter label="Sampai tanggal" name="to" type="date" value={filter.to} />
        <Button type="submit">Saring</Button>
        <Link to="/admin/log-audit" className={buttonVariants({ variant: 'ghost' })}>
          Hapus saringan
        </Link>
      </form>
      {page === 403 ? (
        <Notice error>Anda tidak memiliki izin membaca log audit.</Notice>
      ) : typeof page === 'number' ? (
        <Notice error>Saringan tidak valid.</Notice>
      ) : page.entries.length === 0 ? (
        <Notice>Tidak ada entri yang cocok.</Notice>
      ) : (
        <>
          <DataTable>
            <TableHeader>
              <TableRow>
                <TableHead>Waktu ({zone})</TableHead>
                <TableHead>Pelaku</TableHead>
                <TableHead>Tindakan</TableHead>
                <TableHead>Objek</TableHead>
                <TableHead>Perubahan</TableHead>
                <TableHead>Keterangan</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {page.entries.map((entry) => (
                <TableRow key={entry.id} className="align-top">
                  <TableCell className="text-muted-foreground">
                    {time.format(new Date(entry.at))}
                  </TableCell>
                  <TableCell className="font-medium">{actor(entry)}</TableCell>
                  <TableCell>
                    <Badge variant="outline" className="font-mono">
                      {entry.action}
                    </Badge>
                  </TableCell>
                  <TableCell>
                    {entry.objectType} {entry.objectId}
                  </TableCell>
                  <TableCell>
                    <ul className="grid gap-1">
                      {entry.changes.map((change) => (
                        <li key={change.field}>
                          {change.field}: <del className="text-red">{change.from}</del> →{' '}
                          <ins className="font-medium text-primary no-underline">{change.to}</ins>
                        </li>
                      ))}
                    </ul>
                  </TableCell>
                  <TableCell className="font-mono text-xs">{entry.details}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </DataTable>
          <nav aria-label="Halaman log audit" className="flex justify-end gap-2">
            {before && (
              <Link
                to="/admin/log-audit"
                search={newest}
                className={buttonVariants({ variant: 'outline' })}
              >
                <ChevronsLeft aria-hidden="true" />
                Terbaru
              </Link>
            )}
            {page.next && (
              <Link
                to="/admin/log-audit"
                search={{ ...filter, before: page.next }}
                className={buttonVariants({ variant: 'outline' })}
              >
                Lebih lama
                <ChevronRight aria-hidden="true" />
              </Link>
            )}
          </nav>
        </>
      )}
    </>
  );
}

function Filter({
  label,
  name,
  value,
  type = 'text',
}: {
  label: string;
  name: string;
  value: string | undefined;
  type?: string;
}) {
  return (
    <div className="grid gap-1.5">
      <Label htmlFor={`filter-${name}`} className="text-xs text-muted-foreground">
        {label}
      </Label>
      <Input
        id={`filter-${name}`}
        name={name}
        type={type}
        autoCapitalize="none"
        defaultValue={value}
        className="w-40"
      />
    </div>
  );
}
