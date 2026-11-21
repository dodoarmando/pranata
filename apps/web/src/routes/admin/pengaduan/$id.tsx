import { createFileRoute, getRouteApi, Link, useRouter } from '@tanstack/react-router';
import { createServerFn, useServerFn } from '@tanstack/react-start';
import { ArrowLeft } from 'lucide-react';
import { type FormEvent, useState } from 'react';
import {
  type ComplaintStateChange,
  changeComplaintState,
  getComplaint,
  respondToComplaint,
} from '../../../api/generated';
import { complaintStateLabels, complaintStateVariants } from '../../../complaint';
import { Notice, PageHead } from '../../../components/admin';
import { Badge } from '../../../components/ui/badge';
import { Button } from '../../../components/ui/button';
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '../../../components/ui/dialog';
import { Label } from '../../../components/ui/label';
import { Textarea } from '../../../components/ui/textarea';
import { ianaZones } from '../../../village';

// Server-only; the Complaint, or the status that refused it.
const fetchComplaint = createServerFn({ method: 'GET' })
  .validator((id: string) => id)
  .handler(async ({ data }) => {
    const response = await getComplaint(data);
    return response.status === 200 ? response.data : response.status;
  });

const respondFn = createServerFn({ method: 'POST' })
  .validator((data: { id: string; text: string }) => data)
  .handler(async ({ data }) => (await respondToComplaint(data.id, { text: data.text })).status);

const changeStateFn = createServerFn({ method: 'POST' })
  .validator((data: { id: string; change: ComplaintStateChange }) => data)
  .handler(async ({ data }) => (await changeComplaintState(data.id, data.change)).status);

export const Route = createFileRoute('/admin/pengaduan/$id')({
  loader: ({ params }) => fetchComplaint({ data: params.id }),
  head: () => ({ meta: [{ title: 'Pengaduan · Pranata' }] }),
  component: ComplaintPage,
});

const root = getRouteApi('__root__');

const errors: Record<number, string> = {
  403: 'Anda tidak memiliki izin menangani pengaduan.',
  404: 'Pengaduan ini tidak ditemukan.',
  409: 'Status pengaduan sudah berubah; muat ulang halaman.',
  422: 'Teks wajib diisi, paling banyak 2.000 karakter.',
};

function ComplaintPage() {
  const complaint = Route.useLoaderData();
  const village = root.useLoaderData();
  const router = useRouter();
  const respond = useServerFn(respondFn);
  const changeState = useServerFn(changeStateFn);
  const [message, setMessage] = useState<{ text: string; error: boolean } | null>(null);
  const [rejecting, setRejecting] = useState(false);
  const [dialogError, setDialogError] = useState('');

  const back = (
    <Link
      to="/admin/pengaduan"
      className="flex w-fit items-center gap-1.5 text-muted-foreground hover:text-foreground"
    >
      <ArrowLeft className="size-4" aria-hidden="true" />
      Semua pengaduan
    </Link>
  );
  if (typeof complaint === 'number') {
    return (
      <>
        {back}
        <PageHead title="Pengaduan" />
        <Notice error>
          {complaint === 403
            ? 'Anda tidak memiliki izin membaca pengaduan.'
            : (errors[complaint] ?? 'Gagal memuat pengaduan.')}
        </Notice>
      </>
    );
  }

  const id = complaint.id;
  // Only a User who may handle it answers, closes or rejects an open Complaint.
  const actionable =
    complaint.canHandle && (complaint.state === 'new' || complaint.state === 'in_progress');
  const date = new Intl.DateTimeFormat('id-ID', {
    dateStyle: 'medium',
    timeStyle: 'short',
    timeZone: ianaZones[village?.timeZone ?? 'WIB'],
  });

  /** Shows the outcome; true when it went through. */
  async function done(status: number, text: string) {
    const ok = status < 300;
    setMessage(
      ok ? { text, error: false } : { text: errors[status] ?? 'Gagal menyimpan.', error: true },
    );
    if (ok) await router.invalidate();
    return ok;
  }

  async function onRespond(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const form = event.currentTarget;
    const text = String(new FormData(form).get('text') ?? '');
    const close = (event.nativeEvent as SubmitEvent).submitter?.getAttribute('value') === 'close';
    const status = close
      ? await changeState({ data: { id, change: { state: 'closed', text } } })
      : await respond({ data: { id, text } });
    if (await done(status, close ? 'Pengaduan ditutup.' : 'Tanggapan terkirim.')) form.reset();
  }

  async function onReject(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const text = String(new FormData(event.currentTarget).get('reason') ?? '');
    const status = await changeState({ data: { id, change: { state: 'rejected', text } } });
    if (status >= 300) {
      setDialogError(errors[status] ?? 'Gagal menyimpan.');
      return;
    }
    setRejecting(false);
    await done(status, 'Pengaduan ditolak.');
  }

  return (
    <>
      {back}
      <PageHead title={`Pengaduan ${complaint.code}`} />
      {message && <Notice error={message.error}>{message.text}</Notice>}
      <dl className="grid grid-cols-[auto_1fr] gap-x-6 gap-y-3">
        <dt className="text-muted-foreground">Status</dt>
        <dd>
          <Badge variant={complaintStateVariants[complaint.state]}>
            {complaintStateLabels[complaint.state]}
          </Badge>
        </dd>
        <dt className="text-muted-foreground">Topik</dt>
        <dd>{complaint.topic}</dd>
        <dt className="text-muted-foreground">Dikirim</dt>
        <dd>{date.format(new Date(complaint.submittedAt))}</dd>
        <dt className="text-muted-foreground">Pengirim</dt>
        <dd>{complaint.name ?? 'Anonim'}</dd>
        <dt className="text-muted-foreground">Telepon</dt>
        <dd>{complaint.phone ? <a href={`tel:${complaint.phone}`}>{complaint.phone}</a> : '—'}</dd>
        <dt className="text-muted-foreground">Wilayah</dt>
        <dd>{complaint.hamlet ?? '—'}</dd>
        {complaint.rejectionReason && (
          <>
            <dt className="text-muted-foreground">Alasan ditolak</dt>
            <dd className="whitespace-pre-line">{complaint.rejectionReason}</dd>
          </>
        )}
      </dl>
      <p className="max-w-prose whitespace-pre-line rounded-md border bg-card p-4">
        {complaint.text}
      </p>
      <section aria-labelledby="responses" className="grid max-w-prose gap-4">
        <h2 id="responses" className="text-base font-semibold">
          Tanggapan
        </h2>
        {complaint.responses.length === 0 ? (
          <p className="text-muted-foreground">Belum ada tanggapan.</p>
        ) : (
          <ol className="grid gap-3">
            {complaint.responses.map((response, index) => (
              // Responses are never edited or removed, so their place is stable.
              // biome-ignore lint/suspicious/noArrayIndexKey: see above
              <li key={index} className="grid gap-1 rounded-md border bg-card p-4">
                <span className="text-sm text-muted-foreground">
                  {date.format(new Date(response.at))} · {response.author}
                </span>
                <p className="whitespace-pre-line">{response.text}</p>
              </li>
            ))}
          </ol>
        )}
        {actionable && (
          <form onSubmit={onRespond} className="grid gap-3">
            <div className="grid gap-2">
              <Label htmlFor="response-text">Tanggapan baru</Label>
              <Textarea
                id="response-text"
                name="text"
                required
                maxLength={2000}
                rows={4}
                aria-describedby="response-hint"
              />
              <p id="response-hint" className="text-sm text-muted-foreground">
                Warga membacanya di halaman status pengaduan. Tanggapan tidak dapat diubah atau
                dihapus setelah dikirim.
              </p>
            </div>
            <div className="flex flex-wrap gap-2">
              <Button type="submit" value="respond" size="lg" className="px-4">
                Kirim tanggapan
              </Button>
              <Button type="submit" value="close" variant="outline" size="lg" className="px-4">
                Kirim dan tutup pengaduan
              </Button>
              {complaint.state === 'new' && (
                <Button
                  type="button"
                  variant="outline"
                  size="lg"
                  className="px-4"
                  onClick={async () =>
                    done(
                      await changeState({ data: { id, change: { state: 'in_progress' } } }),
                      'Pengaduan ditandai diproses.',
                    )
                  }
                >
                  Tandai diproses
                </Button>
              )}
              <Button
                type="button"
                variant="destructive"
                size="lg"
                className="px-4"
                onClick={() => {
                  setDialogError('');
                  setRejecting(true);
                }}
              >
                Tolak
              </Button>
            </div>
          </form>
        )}
      </section>
      <Dialog open={rejecting} onOpenChange={setRejecting}>
        <DialogContent>
          <form onSubmit={onReject} className="grid gap-4">
            <DialogHeader>
              <DialogTitle>Tolak pengaduan</DialogTitle>
            </DialogHeader>
            <div className="grid gap-2">
              <Label htmlFor="reject-reason">Alasan</Label>
              <Textarea id="reject-reason" name="reason" required maxLength={2000} rows={3} />
              <p className="text-sm text-muted-foreground">
                Warga membaca alasan ini di halaman status. Pengaduan yang ditolak tidak dapat
                dibuka lagi.
              </p>
            </div>
            {dialogError && <Notice error>{dialogError}</Notice>}
            <DialogFooter>
              <DialogClose
                render={<Button type="button" variant="outline" size="lg" className="px-4" />}
              >
                Batal
              </DialogClose>
              <Button type="submit" variant="destructive" size="lg" className="px-4">
                Tolak pengaduan
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
    </>
  );
}
