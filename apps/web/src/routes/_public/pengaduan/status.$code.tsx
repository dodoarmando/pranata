import { createFileRoute, getRouteApi, Link, notFound } from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import { type ComplaintState, getComplaintStatus } from '../../../api/generated';
import { NotFound } from '../../../components/not-found';
import { privatePage } from '../../../components/public-form';
import { Badge } from '../../../components/ui/badge';
import { newsDate } from '../../../news';
import { pageHead } from '../../../seo';

// Server-only. Null for an unknown code, `throttled` after too many lookups from the address.
const fetchStatus = createServerFn({ method: 'GET' })
  .validator((code: string) => code)
  .handler(async ({ data }) => {
    const response = await getComplaintStatus(data);
    if (response.status === 404) return null;
    if (response.status === 429) return 'throttled' as const;
    if (response.status !== 200) throw new Error('The API could not load the Complaint');
    return response.data;
  });

const stateLabels: Record<ComplaintState, string> = {
  new: 'Diterima',
  in_progress: 'Diproses',
  closed: 'Selesai',
  rejected: 'Ditolak',
};

const root = getRouteApi('__root__');

export const Route = createFileRoute('/_public/pengaduan/status/$code')({
  loader: async ({ params }) => {
    const status = await fetchStatus({ data: params.code });
    if (!status) throw notFound();
    return status;
  },
  head: (context) => {
    const head = pageHead(context, { title: 'Status pengaduan' });
    return { ...head, meta: [...head.meta, ...privatePage.meta] };
  },
  headers: () => privatePage.headers,
  component: ComplaintStatusPage,
  notFoundComponent: () => (
    <NotFound
      title="Pengaduan tidak ditemukan"
      text="Periksa kembali kode pengaduan Anda; kode terdiri atas empat kelompok huruf dan angka."
    />
  ),
});

const heading = 'text-[2rem] leading-tight font-semibold tracking-tight md:text-[2.5rem]';

function ComplaintStatusPage() {
  const village = root.useLoaderData();
  const status = Route.useLoaderData();
  const { code } = Route.useParams();
  const date = (iso: string) => newsDate(iso, village?.timeZone);
  if (status === 'throttled') {
    return (
      <main className="site-container grid max-w-2xl gap-4 pt-10 pb-sec md:pt-16">
        <h1 className={heading}>Status pengaduan</h1>
        <p>Terlalu banyak permintaan dari jaringan Anda. Coba lagi dalam satu menit.</p>
      </main>
    );
  }
  return (
    <main className="site-container grid max-w-2xl gap-8 pt-10 pb-sec md:pt-16">
      <div className="grid gap-3">
        <h1 className={heading}>Status pengaduan</h1>
        <p className="font-mono text-muted-foreground">{code.toUpperCase()}</p>
      </div>
      <dl className="grid grid-cols-[auto_1fr] gap-x-6 gap-y-3">
        <dt className="text-muted-foreground">Topik</dt>
        <dd>{status.topic}</dd>
        <dt className="text-muted-foreground">Dikirim</dt>
        <dd>{date(status.submittedAt)}</dd>
        <dt className="text-muted-foreground">Status</dt>
        <dd>
          <Badge variant={status.state === 'rejected' ? 'destructive' : 'secondary'}>
            {stateLabels[status.state]}
          </Badge>
        </dd>
        {status.rejectionReason && (
          <>
            <dt className="text-muted-foreground">Alasan ditolak</dt>
            <dd className="whitespace-pre-line">{status.rejectionReason}</dd>
          </>
        )}
      </dl>
      <section aria-labelledby="responses" className="grid gap-4">
        <h2 id="responses" className="text-xl font-semibold tracking-tight">
          Tanggapan
        </h2>
        {status.responses.length === 0 ? (
          <p className="text-muted-foreground">Belum ada tanggapan.</p>
        ) : (
          <ol className="grid gap-4">
            {status.responses.map((response, index) => (
              // Responses are never edited or removed, so their place is stable.
              // biome-ignore lint/suspicious/noArrayIndexKey: see above
              <li key={index} className="grid gap-1 rounded-md border bg-card p-4">
                <span className="text-sm text-muted-foreground">{date(response.at)}</span>
                <p className="whitespace-pre-line">{response.text}</p>
              </li>
            ))}
          </ol>
        )}
      </section>
      <Link to="/pengaduan" className="w-fit font-medium text-primary">
        Kirim pengaduan lain
      </Link>
    </main>
  );
}
