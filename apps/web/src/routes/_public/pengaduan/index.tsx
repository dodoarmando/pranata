import { createFileRoute, getRouteApi, Link, useNavigate } from '@tanstack/react-router';
import { createServerFn, useServerFn } from '@tanstack/react-start';
import { type FormEvent, useState } from 'react';
import {
  getComplaintForm,
  getPublicFormChallenge,
  type NewComplaint,
  submitComplaint,
} from '../../../api/generated';
import { Honeypot, privatePage, useProof } from '../../../components/public-form';
import { Alert, AlertDescription } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { Input } from '../../../components/ui/input';
import { Label } from '../../../components/ui/label';
import { NativeSelect } from '../../../components/ui/native-select';
import { Textarea } from '../../../components/ui/textarea';
import { pageHead } from '../../../seo';
import { capitalized, villageTitle } from '../../../village';

// Server-only: the browser never reaches the API. A fresh challenge with every load.
const fetchComplaintForm = createServerFn({ method: 'GET' }).handler(async () => {
  const [form, challenge] = await Promise.all([getComplaintForm(), getPublicFormChallenge()]);
  if (form.status !== 200 || challenge.status !== 200) {
    throw new Error('The API could not load the Complaint form');
  }
  return { ...form.data, challenge: challenge.data };
});

// The code, or the status of a refusal: 422 for a field to fix.
const submitComplaintFn = createServerFn({ method: 'POST' })
  .validator((data: NewComplaint) => data)
  .handler(async ({ data }) => {
    const response = await submitComplaint(data);
    return response.status === 201
      ? { code: response.data.code }
      : { code: null, status: response.status };
  });

const root = getRouteApi('__root__');

export const Route = createFileRoute('/_public/pengaduan/')({
  loader: () => fetchComplaintForm(),
  head: (context) => {
    const head = pageHead(context, {
      title: 'Pengaduan',
      description: 'Sampaikan pengaduan atau saran kepada pemerintah desa.',
    });
    return { ...head, meta: [...head.meta, ...privatePage.meta] };
  },
  headers: () => privatePage.headers,
  component: ComplaintPage,
});

const heading = 'text-[2rem] leading-tight font-semibold tracking-tight md:text-[2.5rem]';

function ComplaintPage() {
  const village = root.useLoaderData();
  const { topics, hamletLabel, hamlets, challenge } = Route.useLoaderData();
  const proof = useProof(challenge);
  const send = useServerFn(submitComplaintFn);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [code, setCode] = useState<string | null>(null);
  const hamlet = capitalized(hamletLabel);

  async function onSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!proof) return;
    const form = new FormData(event.currentTarget);
    const optional = (name: string) => String(form.get(name) ?? '').trim() || null;
    setBusy(true);
    setError('');
    const result = await send({
      data: {
        topicId: String(form.get('topic')),
        text: String(form.get('text')),
        hamletId: optional('hamlet'),
        name: optional('name'),
        phone: optional('phone'),
        website: String(form.get('website') ?? ''),
        proof,
      },
    }).catch(() => ({ code: null, status: 0 }));
    setBusy(false);
    if (result.code) {
      setCode(result.code);
      scrollTo({ top: 0 });
    } else if (result.status === 422) {
      setError('Periksa isian Anda. Nomor telepon hanya boleh berisi angka, spasi, dan + - ( ).');
    } else {
      setError('Pengaduan belum terkirim. Coba lagi beberapa saat lagi.');
    }
  }

  if (code) {
    return (
      <main className="site-container grid max-w-2xl gap-6 pt-10 pb-sec md:pt-16">
        <h1 className={heading}>Pengaduan terkirim</h1>
        <p>
          Terima kasih. Simpan kode pengaduan berikut; hanya dengan kode ini Anda dapat melihat
          status dan tanggapan atas pengaduan Anda.
        </p>
        <p className="w-fit rounded-md border bg-card px-4 py-3 font-mono text-2xl font-semibold tracking-wider">
          {code}
        </p>
        <Link
          to="/pengaduan/status/$code"
          params={{ code }}
          className="w-fit font-medium text-primary"
        >
          Lihat status pengaduan
        </Link>
      </main>
    );
  }

  return (
    <main className="site-container grid max-w-2xl gap-10 pt-10 pb-sec md:pt-16">
      <div className="grid gap-3">
        <h1 className={heading}>Pengaduan</h1>
        <p className="text-muted-foreground">
          Sampaikan pengaduan atau saran kepada pemerintah desa. Anda boleh tidak mencantumkan nama.
        </p>
      </div>
      <form method="post" onSubmit={onSubmit} className="relative grid gap-5">
        <div className="grid gap-2">
          <Label htmlFor="topic">Topik</Label>
          <NativeSelect id="topic" name="topic" required defaultValue="" className="w-full">
            <option value="" disabled>
              Pilih topik
            </option>
            {topics.map((topic) => (
              <option key={topic.id} value={topic.id}>
                {topic.name}
              </option>
            ))}
          </NativeSelect>
        </div>
        <div className="grid gap-2">
          <Label htmlFor="text">Isi pengaduan atau saran</Label>
          <Textarea id="text" name="text" required maxLength={2000} rows={6} />
          <p className="text-sm text-muted-foreground">Paling banyak 2.000 karakter.</p>
        </div>
        {hamlets.length > 0 && (
          <div className="grid gap-2">
            <Label htmlFor="hamlet">{hamlet} (tidak wajib)</Label>
            <NativeSelect id="hamlet" name="hamlet" defaultValue="" className="w-full">
              <option value="">Tidak disebutkan</option>
              {hamlets.map((item) => (
                <option key={item.id} value={item.id}>
                  {hamlet} {item.name}
                </option>
              ))}
            </NativeSelect>
          </div>
        )}
        <div className="grid gap-2">
          <Label htmlFor="name">Nama (tidak wajib)</Label>
          <Input id="name" name="name" maxLength={100} autoComplete="name" />
        </div>
        <div className="grid gap-2">
          <Label htmlFor="phone">Nomor telepon (tidak wajib)</Label>
          <Input id="phone" name="phone" type="tel" maxLength={20} autoComplete="tel" />
        </div>
        <Honeypot />
        <p className="text-sm text-muted-foreground">
          Pengaduan diproses oleh Pemerintah {village ? villageTitle(village) : 'Desa'} dalam rangka
          pelayanan publik. Nama dan nomor telepon hanya dilihat petugas desa dan tidak ditampilkan
          di halaman status. Data disimpan sesuai jadwal retensi arsip desa.
        </p>
        {error && (
          <Alert variant="destructive">
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        )}
        <Button type="submit" size="lg" className="h-11 w-fit px-5" disabled={!proof || busy}>
          {!proof ? 'Memverifikasi…' : busy ? 'Mengirim…' : 'Kirim pengaduan'}
        </Button>
      </form>
      <StatusLookup />
    </main>
  );
}

/** Where a sender types the Complaint Code to reach its status page. */
function StatusLookup() {
  const navigate = useNavigate();
  function onSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const code = String(new FormData(event.currentTarget).get('code')).trim().toUpperCase();
    if (code) void navigate({ to: '/pengaduan/status/$code', params: { code } });
  }
  return (
    <section aria-labelledby="status-lookup" className="grid gap-3 border-t pt-8">
      <h2 id="status-lookup" className="text-xl font-semibold tracking-tight">
        Sudah mengirim pengaduan?
      </h2>
      <form onSubmit={onSubmit} className="flex flex-wrap items-end gap-3">
        <div className="grid flex-1 gap-2">
          <Label htmlFor="code">Kode pengaduan</Label>
          <Input id="code" name="code" required autoCapitalize="characters" />
        </div>
        <Button type="submit" variant="outline" size="lg" className="h-11 px-5">
          Cek status
        </Button>
      </form>
    </section>
  );
}
