import { createFileRoute, Link, useNavigate } from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import { calendarDate } from '../../announcement';
import {
  bidang,
  netFinancing,
  rupiah,
  share,
  spending,
  surplus,
  versionLabels,
} from '../../apbdes-summary';
import {
  type BudgetLines,
  getPublishedApbdesSummary,
  type PublishedApbdesSummary,
} from '../../api/generated';
import { TransparencyDocumentItem } from '../../components/transparency-document-item';
import { NativeSelect, NativeSelectOption } from '../../components/ui/native-select';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '../../components/ui/table';
import { pageHead } from '../../seo';

type Search = { tahun?: number };

// Server-only; null when the year has no Published summary or the API cannot be reached.
const fetchSummary = createServerFn({ method: 'GET' })
  .validator((search: Search) => search)
  .handler(async ({ data }): Promise<PublishedApbdesSummary | null> => {
    try {
      const response = await getPublishedApbdesSummary(
        data.tahun ? { fiscalYear: data.tahun } : {},
      );
      return response.status === 200 ? response.data : null;
    } catch {
      return null;
    }
  });

export const Route = createFileRoute('/_public/apbdes')({
  validateSearch: (search: Record<string, unknown>): Search => {
    const year = Number(search.tahun);
    return Number.isInteger(year) && year >= 2000 && year <= 2100 ? { tahun: year } : {};
  },
  loaderDeps: ({ search }) => search,
  loader: ({ deps }) => fetchSummary({ data: deps }),
  head: (context) =>
    pageHead(context, {
      title: context.loaderData ? `APBDes ${context.loaderData.fiscalYear}` : 'APBDes',
    }),
  component: ApbdesPage,
});

/** A row of the breakdown: its label and how it reads from one set of lines. */
type Row = {
  label: string;
  amount: (lines: BudgetLines) => number;
  total?: boolean;
  // A difference of two lines, so a share realised says nothing.
  noShare?: boolean;
};

const sections: { heading: string; rows: Row[] }[] = [
  { heading: 'Pendapatan', rows: [{ label: 'Pendapatan', amount: (l) => l.revenue }] },
  {
    heading: 'Belanja',
    rows: [
      ...bidang.map(({ line, label }) => ({ label, amount: (l: BudgetLines) => l[line] })),
      { label: 'Jumlah belanja', amount: spending, total: true },
    ],
  },
  {
    heading: 'Surplus / defisit',
    rows: [{ label: 'Surplus / defisit', amount: surplus, noShare: true }],
  },
  {
    heading: 'Pembiayaan',
    rows: [
      { label: 'Penerimaan pembiayaan', amount: (l) => l.financingReceipts },
      { label: 'Pengeluaran pembiayaan', amount: (l) => l.financingExpenditures },
      { label: 'Pembiayaan netto', amount: netFinancing, total: true },
    ],
  },
];

function ApbdesPage() {
  const summary = Route.useLoaderData();
  const search = Route.useSearch();
  const navigate = useNavigate({ from: '/apbdes' });

  if (!summary) {
    return (
      <main className="site-container grid gap-4 pt-10 pb-sec md:pt-16">
        <h1 className="text-[2rem] leading-tight font-semibold tracking-tight md:text-[2.5rem]">
          APBDes
        </h1>
        <p className="text-muted-foreground">
          {search.tahun
            ? `Ringkasan APBDes tahun anggaran ${search.tahun} belum diterbitkan.`
            : 'Ringkasan APBDes belum diterbitkan.'}{' '}
          <Link to="/dokumen" className="font-medium text-primary">
            Lihat dokumen transparansi
          </Link>
        </p>
      </main>
    );
  }

  const { budget, realisation } = summary;
  const stats = [
    { label: 'Pendapatan', amount: budget.revenue, realised: realisation?.revenue },
    {
      label: 'Belanja',
      amount: spending(budget),
      realised: realisation ? spending(realisation) : undefined,
    },
    { label: 'Pembiayaan netto', amount: netFinancing(budget) },
  ];
  return (
    <main className="site-container grid gap-10 pt-10 pb-sec md:pt-16">
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div className="grid max-w-2xl gap-3">
          <p className="text-sm font-medium text-primary">Transparansi anggaran</p>
          <h1 className="text-[2rem] leading-tight font-semibold tracking-tight md:text-[2.5rem]">
            APBDes Tahun Anggaran {summary.fiscalYear}
          </h1>
          <p className="text-muted-foreground md:text-lg">
            Versi {versionLabels[summary.version]}, ditetapkan {calendarDate(summary.versionDate)}.
            {realisation && ` Realisasi per ${calendarDate(realisation.asOf)}.`}
          </p>
        </div>
        {summary.fiscalYears.length > 1 && (
          <div className="flex items-center gap-2">
            <label htmlFor="fiscal-year" className="text-sm text-muted-foreground">
              Tahun anggaran
            </label>
            <NativeSelect
              id="fiscal-year"
              value={summary.fiscalYear}
              onChange={(event) =>
                navigate({ search: { tahun: Number(event.currentTarget.value) } })
              }
            >
              {summary.fiscalYears.map((year) => (
                <NativeSelectOption key={year} value={year}>
                  {year}
                </NativeSelectOption>
              ))}
            </NativeSelect>
          </div>
        )}
      </div>
      <dl className="grid gap-4 sm:grid-cols-3">
        {stats.map(({ label, amount, realised }) => (
          <div key={label} className="grid gap-1 rounded-md border bg-card p-4">
            <dt className="text-sm text-muted-foreground">{label}</dt>
            <dd className="text-2xl font-semibold tabular-nums">{rupiah(amount)}</dd>
            {realised !== undefined && (
              <dd className="text-sm text-muted-foreground">
                Realisasi {share(realised, amount)}%
              </dd>
            )}
          </div>
        ))}
      </dl>
      <section aria-labelledby="rincian" className="grid gap-4">
        <h2 id="rincian" className="text-2xl font-semibold tracking-tight">
          Rincian anggaran
        </h2>
        <div className="overflow-x-auto rounded-md border bg-card">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Uraian</TableHead>
                <TableHead className="text-right">Anggaran</TableHead>
                {realisation && (
                  <>
                    <TableHead className="text-right">Realisasi</TableHead>
                    <TableHead className="text-right">%</TableHead>
                  </>
                )}
              </TableRow>
            </TableHeader>
            {sections.map(({ heading, rows }) => (
              <TableBody key={heading}>
                {rows.length > 1 && (
                  <TableRow>
                    <TableHead colSpan={realisation ? 4 : 2} scope="colgroup">
                      {heading}
                    </TableHead>
                  </TableRow>
                )}
                {rows.map(({ label, amount, total, noShare }) => (
                  <TableRow key={label} className={total ? 'font-semibold' : undefined}>
                    <TableCell className="whitespace-normal">{label}</TableCell>
                    <TableCell className="text-right tabular-nums">
                      {rupiah(amount(budget))}
                    </TableCell>
                    {realisation && (
                      <>
                        <TableCell className="text-right tabular-nums">
                          {rupiah(amount(realisation))}
                        </TableCell>
                        <TableCell className="text-right tabular-nums">
                          {noShare ? '–' : `${share(amount(realisation), amount(budget))}%`}
                        </TableCell>
                      </>
                    )}
                  </TableRow>
                ))}
              </TableBody>
            ))}
          </Table>
        </div>
      </section>
      <section aria-labelledby="dokumen" className="grid gap-4">
        <h2 id="dokumen" className="text-2xl font-semibold tracking-tight">
          Dokumen APBDes {summary.fiscalYear}
        </h2>
        {summary.documents.length ? (
          <ul className="grid gap-3">
            {summary.documents.map((document, index) => (
              // A static list the server orders; a place is unique where titles may repeat.
              // biome-ignore lint/suspicious/noArrayIndexKey: see above
              <TransparencyDocumentItem key={index} document={document} heading="h3" />
            ))}
          </ul>
        ) : (
          <p className="text-muted-foreground">Belum ada dokumen anggaran tahun ini.</p>
        )}
      </section>
    </main>
  );
}
