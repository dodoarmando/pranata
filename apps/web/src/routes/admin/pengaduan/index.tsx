import { createFileRoute, getRouteApi, Link } from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import { type ComplaintState, listComplaints } from '../../../api/generated';
import { complaintStateLabels, complaintStateVariants } from '../../../complaint';
import { DataTable, Notice, PageHead } from '../../../components/admin';
import { Badge } from '../../../components/ui/badge';
import { buttonVariants } from '../../../components/ui/button';
import {
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '../../../components/ui/table';
import { ianaZones } from '../../../village';

type Filter = { state?: ComplaintState };

const filterOf = (search: Record<string, unknown>): Filter =>
  typeof search.state === 'string' && search.state in complaintStateLabels
    ? { state: search.state as ComplaintState }
    : {};

// Server-only; the queue, or the status that refused it.
const fetchComplaints = createServerFn({ method: 'GET' })
  .validator(filterOf)
  .handler(async ({ data }) => {
    const response = await listComplaints(data);
    return response.status === 200 ? response.data : response.status;
  });

export const Route = createFileRoute('/admin/pengaduan/')({
  validateSearch: filterOf,
  loaderDeps: ({ search }) => search,
  loader: ({ deps }) => fetchComplaints({ data: deps }),
  head: () => ({ meta: [{ title: 'Pengaduan · Pranata' }] }),
  component: Complaints,
});

const root = getRouteApi('__root__');

const tabs: { label: string; state?: ComplaintState }[] = [
  { label: 'Semua' },
  ...Object.entries(complaintStateLabels).map(([state, label]) => ({
    label,
    state: state as ComplaintState,
  })),
];

function Complaints() {
  const complaints = Route.useLoaderData();
  const filter = Route.useSearch();
  const village = root.useLoaderData();
  const date = new Intl.DateTimeFormat('id-ID', {
    dateStyle: 'medium',
    timeStyle: 'short',
    timeZone: ianaZones[village?.timeZone ?? 'WIB'],
  });

  return (
    <>
      <PageHead title="Pengaduan">
        Pengaduan dan saran dari warga. Tanggapan tampil di halaman status yang dibuka warga dengan
        kode pengaduannya.
      </PageHead>
      {typeof complaints === 'number' ? (
        <Notice error>
          {complaints === 403
            ? 'Anda tidak memiliki izin membaca pengaduan.'
            : 'Gagal memuat pengaduan.'}
        </Notice>
      ) : (
        <>
          <div className="flex flex-wrap items-center justify-between gap-3">
            <nav aria-label="Saring menurut status" className="flex flex-wrap gap-2">
              {tabs.map((tab) => (
                <Link
                  key={tab.label}
                  to="/admin/pengaduan"
                  search={tab.state ? { state: tab.state } : {}}
                  aria-current={filter.state === tab.state ? 'page' : undefined}
                  className={buttonVariants({
                    variant: filter.state === tab.state ? 'default' : 'outline',
                  })}
                >
                  {tab.label}
                </Link>
              ))}
            </nav>
            <Link to="/admin/pengaduan/topik" className={buttonVariants({ variant: 'outline' })}>
              Kelola topik
            </Link>
          </div>
          {complaints.length === 0 ? (
            <p className="text-muted-foreground">Tidak ada pengaduan.</p>
          ) : (
            <DataTable>
              <TableHeader>
                <TableRow>
                  <TableHead>Isi</TableHead>
                  <TableHead>Topik</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead>Dikirim</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {complaints.map((complaint) => (
                  <TableRow key={complaint.id}>
                    <TableCell className="max-w-96 truncate">
                      <Link
                        to="/admin/pengaduan/$id"
                        params={{ id: complaint.id }}
                        className="font-medium underline-offset-4 hover:underline"
                      >
                        {complaint.text}
                      </Link>
                    </TableCell>
                    <TableCell>{complaint.topic}</TableCell>
                    <TableCell>
                      <Badge variant={complaintStateVariants[complaint.state]}>
                        {complaintStateLabels[complaint.state]}
                      </Badge>
                    </TableCell>
                    <TableCell>{date.format(new Date(complaint.submittedAt))}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </DataTable>
          )}
        </>
      )}
    </>
  );
}
