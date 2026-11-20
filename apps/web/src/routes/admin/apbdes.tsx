import { createFileRoute, getRouteApi, useRouter } from '@tanstack/react-router';
import { createServerFn, useServerFn } from '@tanstack/react-start';
import { Plus } from 'lucide-react';
import { type FormEvent, type ReactNode, useState } from 'react';
import { calendarDate } from '../../announcement';
import { bidang, netFinancing, rupiah, share, spending, versionLabels } from '../../apbdes-summary';
import {
  type ApbdesSummary,
  type ApbdesSummaryFields,
  type BudgetLines,
  type BudgetVersion,
  createApbdesSummary,
  deleteApbdesSummary,
  listApbdesSummaries,
  publishApbdesSummary,
  unpublishApbdesSummary,
  updateApbdesSummary,
} from '../../api/generated';
import { useAutosave } from '../../autosave';
import { DataTable, Notice, PageHead } from '../../components/admin';
import { Badge } from '../../components/ui/badge';
import { Button } from '../../components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '../../components/ui/card';
import { Input } from '../../components/ui/input';
import { Label } from '../../components/ui/label';
import { NativeSelect, NativeSelectOption } from '../../components/ui/native-select';
import { TableBody, TableCell, TableHead, TableHeader, TableRow } from '../../components/ui/table';
import { ianaZones } from '../../village';

// Server-only; null without a content Permission.
const fetchSummaries = createServerFn({ method: 'GET' }).handler(async () => {
  const response = await listApbdesSummaries();
  return {
    summaries: response.status === 200 ? response.data : null,
    forbidden: response.status === 403,
  };
});

const createSummaryFn = createServerFn({ method: 'POST' })
  .validator((summary: ApbdesSummaryFields) => summary)
  .handler(async ({ data }) => {
    const response = await createApbdesSummary(data);
    return response.status === 201
      ? { status: response.status, id: response.data.id }
      : { status: response.status };
  });

const updateSummaryFn = createServerFn({ method: 'POST' })
  .validator((data: { id: string; summary: ApbdesSummaryFields }) => data)
  .handler(async ({ data }) => (await updateApbdesSummary(data.id, data.summary)).status);

const publishFn = createServerFn({ method: 'POST' })
  .validator((data: { id: string; publish: boolean }) => data)
  .handler(
    async ({ data }) =>
      (await (data.publish ? publishApbdesSummary(data.id) : unpublishApbdesSummary(data.id)))
        .status,
  );

const deleteSummaryFn = createServerFn({ method: 'POST' })
  .validator((id: string) => id)
  .handler(async ({ data }) => (await deleteApbdesSummary(data)).status);

export const Route = createFileRoute('/admin/apbdes')({
  loader: () => fetchSummaries(),
  head: () => ({ meta: [{ title: 'Ringkasan APBDes · Pranata' }] }),
  component: Summaries,
});

const summaryErrors: Record<number, string> = {
  403: 'Hanya pengguna dengan izin menerbitkan konten yang dapat melakukannya; ringkasan yang sudah terbit hanya dapat diubah oleh penerbit.',
  404: 'Ringkasan ini sudah dihapus.',
  409: 'Tahun anggaran ini sudah punya ringkasan, atau statusnya sudah berubah; muat ulang.',
  422: 'Periksa lagi isian: tahun anggaran antara 2000 dan 2100, dan setiap jumlah berupa rupiah penuh yang tidak negatif.',
};

const noLines: BudgetLines = {
  revenue: 0,
  governance: 0,
  development: 0,
  community: 0,
  empowerment: 0,
  emergencies: 0,
  financingReceipts: 0,
  financingExpenditures: 0,
};

const today = () => new Date().toISOString().slice(0, 10);

const root = getRouteApi('__root__');

function Summaries() {
  const { summaries, forbidden } = Route.useLoaderData();
  const router = useRouter();
  const createSummary = useServerFn(createSummaryFn);
  const setPublished = useServerFn(publishFn);
  const removeSummary = useServerFn(deleteSummaryFn);
  const [message, setMessage] = useState<{ text: string; error: boolean } | null>(null);
  // The summary open in the form; null while the list shows.
  const [editing, setEditing] = useState<ApbdesSummary | null>(null);

  if (!summaries) {
    return (
      <>
        <PageHead title="Ringkasan APBDes" />
        <Notice error>
          {forbidden ? 'Anda tidak memiliki izin menulis konten.' : 'Gagal memuat ringkasan.'}
        </Notice>
      </>
    );
  }

  async function result(status: number, done: string) {
    if (status >= 300) {
      setMessage({ text: summaryErrors[status] ?? 'Gagal menyimpan.', error: true });
      return;
    }
    setMessage({ text: done, error: false });
    await router.invalidate();
  }

  const open = (summary: ApbdesSummary) => {
    setMessage(null);
    setEditing(summary);
  };

  // A new year starts as a Draft right away, so that the form autosaves into it.
  const onAdd = async () => {
    const fields: ApbdesSummaryFields = {
      fiscalYear: Math.max(new Date().getFullYear(), ...summaries.map((s) => s.fiscalYear + 1)),
      version: 'original',
      versionDate: today(),
      budget: noLines,
      realisation: null,
    };
    const created = await createSummary({ data: fields });
    if (!created.id) {
      setMessage({ text: summaryErrors[created.status] ?? 'Gagal menyimpan.', error: true });
      return;
    }
    open({ id: created.id, ...fields, published: false });
    await router.invalidate();
  };

  async function onDelete(summary: ApbdesSummary) {
    if (!window.confirm(`Hapus draf ringkasan APBDes ${summary.fiscalYear}?`)) return;
    await result(await removeSummary({ data: summary.id }), 'Ringkasan dihapus.');
  }

  if (editing) {
    return <SummaryForm key={editing.id} summary={editing} onClose={() => setEditing(null)} />;
  }

  return (
    <>
      <PageHead title="Ringkasan APBDes">
        Satu ringkasan per tahun anggaran. Setelah diterbitkan, warga membacanya di halaman APBDes
        situs bersama dokumen APBDes tahun yang sama.
      </PageHead>
      {message && <Notice error={message.error}>{message.text}</Notice>}
      <Button type="button" size="lg" className="w-fit px-4" onClick={onAdd}>
        <Plus aria-hidden="true" />
        Tambah tahun anggaran
      </Button>
      {summaries.length === 0 ? (
        <p className="text-muted-foreground">Belum ada ringkasan.</p>
      ) : (
        <DataTable>
          <TableHeader>
            <TableRow>
              <TableHead>Tahun anggaran</TableHead>
              <TableHead>Versi</TableHead>
              <TableHead>Pendapatan</TableHead>
              <TableHead>Belanja</TableHead>
              <TableHead>Realisasi</TableHead>
              <TableHead>Status</TableHead>
              <TableHead>
                <span className="sr-only">Tindakan</span>
              </TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {summaries.map((summary) => (
              <TableRow key={summary.id}>
                <TableCell className="font-medium">{summary.fiscalYear}</TableCell>
                <TableCell>
                  {versionLabels[summary.version]} · {calendarDate(summary.versionDate)}
                </TableCell>
                <TableCell>{rupiah(summary.budget.revenue)}</TableCell>
                <TableCell>{rupiah(spending(summary.budget))}</TableCell>
                <TableCell>
                  {summary.realisation ? `per ${calendarDate(summary.realisation.asOf)}` : '–'}
                </TableCell>
                <TableCell>
                  <Badge variant={summary.published ? 'default' : 'secondary'}>
                    {summary.published ? 'Terbit' : 'Draf'}
                  </Badge>
                </TableCell>
                <TableCell>
                  <div className="flex justify-end gap-2">
                    <Button type="button" variant="outline" onClick={() => open(summary)}>
                      Ubah
                    </Button>
                    <Button
                      type="button"
                      variant="outline"
                      onClick={async () =>
                        result(
                          await setPublished({
                            data: { id: summary.id, publish: !summary.published },
                          }),
                          summary.published
                            ? 'Ringkasan kembali menjadi draf.'
                            : 'Ringkasan terbit di halaman APBDes.',
                        )
                      }
                    >
                      {summary.published ? 'Batalkan terbit' : 'Terbitkan'}
                    </Button>
                    {!summary.published && (
                      <Button type="button" variant="outline" onClick={() => onDelete(summary)}>
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
    </>
  );
}

/** One summary's fields: a Draft saves itself as the writer pauses, a Published one on Simpan. */
function SummaryForm({ summary, onClose }: { summary: ApbdesSummary; onClose: () => void }) {
  const village = root.useLoaderData();
  const updateSummary = useServerFn(updateSummaryFn);
  const [error, setError] = useState('');
  const { id, published, ...initial } = summary;
  const { fields, change, dirty, saving, savedAt, flush } = useAutosave<ApbdesSummaryFields>(
    () => initial,
    published,
    (saved) => updateSummary({ data: { id, summary: saved } }),
    (result) => setError(result >= 300 ? (summaryErrors[result] ?? 'Gagal menyimpan.') : ''),
  );
  const time = new Intl.DateTimeFormat('id-ID', {
    timeStyle: 'short',
    timeZone: ianaZones[village?.timeZone ?? 'WIB'],
  });
  const status = published
    ? dirty
      ? 'Ada perubahan yang belum disimpan.'
      : 'Terbit di situs.'
    : saving
      ? 'Menyimpan…'
      : dirty
        ? 'Perubahan akan tersimpan otomatis.'
        : savedAt
          ? `Tersimpan otomatis pukul ${time.format(savedAt)}.`
          : 'Draf, belum tampil di situs.';

  async function onDone(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (dirty && !(await flush())) return;
    onClose();
  }

  const realisation = fields.realisation ?? null;
  return (
    <form onSubmit={onDone} className="grid gap-6">
      <PageHead title={`APBDes ${fields.fiscalYear}`}>
        Total APBDes yang diisi manual, dalam rupiah penuh.
      </PageHead>
      <div className="flex flex-wrap items-center gap-3">
        <Badge variant={published ? 'default' : 'secondary'}>{published ? 'Terbit' : 'Draf'}</Badge>
        <p role="status" className="text-muted-foreground">
          {status}
        </p>
      </div>
      <Card>
        <CardHeader>
          <CardTitle>Versi</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-wrap gap-4">
          <div className="grid gap-2">
            <Label htmlFor="apbdes-year">Tahun anggaran</Label>
            <Input
              id="apbdes-year"
              type="number"
              required
              min={2000}
              max={2100}
              className="w-28"
              value={fields.fiscalYear}
              onChange={(e) => change({ fiscalYear: Number(e.currentTarget.value) })}
            />
          </div>
          <div className="grid gap-2">
            <Label htmlFor="apbdes-version">Versi</Label>
            <NativeSelect
              id="apbdes-version"
              value={fields.version}
              onChange={(e) => change({ version: e.currentTarget.value as BudgetVersion })}
            >
              {(Object.keys(versionLabels) as BudgetVersion[]).map((version) => (
                <NativeSelectOption key={version} value={version}>
                  {versionLabels[version]}
                </NativeSelectOption>
              ))}
            </NativeSelect>
          </div>
          <div className="grid gap-2">
            <Label htmlFor="apbdes-version-date">Tanggal ditetapkan</Label>
            <Input
              id="apbdes-version-date"
              type="date"
              required
              value={fields.versionDate}
              onChange={(e) => change({ versionDate: e.currentTarget.value })}
            />
          </div>
        </CardContent>
      </Card>
      <LinesCard
        title="Anggaran"
        id="budget"
        lines={fields.budget}
        onChange={(budget) => change({ budget })}
      />
      <label className="flex items-center gap-2.5">
        <input
          type="checkbox"
          checked={realisation !== null}
          onChange={(e) =>
            change({
              realisation: e.currentTarget.checked ? { asOf: today(), ...noLines } : null,
            })
          }
          className="size-4 accent-primary"
        />
        Sudah ada realisasi
      </label>
      {realisation && (
        <LinesCard
          title="Realisasi"
          id="realised"
          lines={realisation}
          onChange={(lines) => change({ realisation: { ...realisation, ...lines } })}
        >
          <div className="grid gap-2">
            <Label htmlFor="realised-as-of">Realisasi per tanggal</Label>
            <Input
              id="realised-as-of"
              type="date"
              required
              className="w-fit"
              value={realisation.asOf}
              onChange={(e) =>
                change({ realisation: { ...realisation, asOf: e.currentTarget.value } })
              }
            />
          </div>
        </LinesCard>
      )}
      {error && <Notice error>{error}</Notice>}
      <div className="flex gap-2">
        <Button type="submit" size="lg" className="px-4" disabled={saving}>
          {published ? 'Simpan' : 'Selesai'}
        </Button>
        {published && (
          <Button type="button" variant="outline" size="lg" className="px-4" onClick={onClose}>
            Batal
          </Button>
        )}
      </div>
    </form>
  );
}

/** Revenue, spending per bidang and financing, with their computed totals. */
function LinesCard({
  title,
  id,
  lines,
  onChange,
  children,
}: {
  title: string;
  id: string;
  lines: BudgetLines;
  onChange: (lines: BudgetLines) => void;
  children?: ReactNode;
}) {
  const total = spending(lines);
  const amount = (line: keyof BudgetLines, label: string, extra?: ReactNode) => (
    <div key={line} className="flex flex-wrap items-center gap-x-4 gap-y-2">
      <Label htmlFor={`${id}-${line}`} className="flex-[1_1_16rem] font-normal">
        {label}
      </Label>
      <Input
        id={`${id}-${line}`}
        type="number"
        required
        min={0}
        max={999_999_999_999_999}
        step={1}
        className="w-48 text-right tabular-nums"
        value={lines[line]}
        onChange={(e) => onChange({ ...lines, [line]: Number(e.currentTarget.value) })}
      />
      {extra}
    </div>
  );
  const totalRow = (label: string, value: number) => (
    <p className="flex justify-between border-t pt-3 font-medium">
      <span>{label}</span>
      <span className="tabular-nums">{rupiah(value)}</span>
    </p>
  );
  return (
    <Card>
      <CardHeader>
        <CardTitle>{title}</CardTitle>
      </CardHeader>
      <CardContent className="grid gap-4">
        {children}
        {amount('revenue', 'Pendapatan')}
        <h3 className="font-medium">Belanja per bidang</h3>
        {bidang.map(({ line, label }) =>
          amount(
            line,
            label,
            <span className="w-12 text-right text-muted-foreground tabular-nums">
              {share(lines[line], total)}%
            </span>,
          ),
        )}
        {totalRow('Jumlah belanja', total)}
        <h3 className="font-medium">Pembiayaan</h3>
        {amount('financingReceipts', 'Penerimaan pembiayaan')}
        {amount('financingExpenditures', 'Pengeluaran pembiayaan')}
        {totalRow('Pembiayaan netto', netFinancing(lines))}
      </CardContent>
    </Card>
  );
}
