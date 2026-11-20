import { createFileRoute, getRouteApi } from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import {
  getVisionMission,
  type IndexEdition,
  listIndexEditions,
  type VisionMission,
} from '../../api/generated';
import { dimensions, editionLabel, formatScore, scales, statusLabels } from '../../index-edition';
import { pageHead } from '../../seo';
import { villageCodeDotted, villageTitle } from '../../village';

// Server-only; null until saved, or when the API cannot be reached.
const fetchVisionMission = createServerFn({ method: 'GET' }).handler(
  async (): Promise<VisionMission | null> => {
    try {
      const response = await getVisionMission();
      return response.status === 200 ? response.data : null;
    } catch {
      return null;
    }
  },
);

// Server-only; empty when the API cannot be reached.
const fetchEditions = createServerFn({ method: 'GET' }).handler(
  async (): Promise<IndexEdition[]> => {
    try {
      const response = await listIndexEditions();
      return response.status === 200 ? response.data : [];
    } catch {
      return [];
    }
  },
);

const root = getRouteApi('__root__');

export const Route = createFileRoute('/_public/profil')({
  loader: async () => {
    const [visionMission, editions] = await Promise.all([fetchVisionMission(), fetchEditions()]);
    return { visionMission, editions };
  },
  head: (context) => pageHead(context, { title: 'Profil' }),
  component: ProfilePage,
});

function ProfilePage() {
  const village = root.useLoaderData();
  const { visionMission, editions } = Route.useLoaderData();
  // The newest year first, so the first is the latest Edition.
  const latest = editions[0];
  const scored = latest ? dimensions.filter(([key]) => latest[key] != null) : [];
  return (
    <main className="site-container grid gap-12 pt-10 pb-sec md:pt-16">
      <section aria-labelledby="profil-title" className="grid gap-6">
        <h1
          id="profil-title"
          className="text-[2rem] leading-tight font-semibold tracking-tight md:text-[2.5rem]"
        >
          {village ? `Profil ${villageTitle(village)}` : 'Profil'}
        </h1>
        {village ? (
          <dl className="grid max-w-xl rounded-md border bg-card px-5 py-2">
            {[
              ['Kode desa', villageCodeDotted(village.code)],
              ['Kecamatan', village.district],
              ['Kabupaten/Kota', village.regency],
              ['Provinsi', village.province],
            ].map(([term, value]) => (
              <div key={term} className="flex justify-between gap-3 border-b py-3 last:border-b-0">
                <dt className="text-sm text-muted-foreground">{term}</dt>
                <dd className="text-right font-semibold">{value}</dd>
              </div>
            ))}
          </dl>
        ) : (
          <p className="text-muted-foreground">Profil desa belum tersedia.</p>
        )}
      </section>
      <section id="visi-misi" aria-labelledby="visi-misi-title" className="grid gap-6">
        <div>
          <h2 id="visi-misi-title" className="text-2xl font-semibold tracking-tight md:text-3xl">
            Visi dan misi
          </h2>
          {visionMission && (
            <p className="mt-1 text-muted-foreground">
              RPJMDes {visionMission.startYear}–{visionMission.endYear}
            </p>
          )}
        </div>
        {visionMission ? (
          <div className="grid items-start gap-6 md:grid-cols-2">
            <div className="rounded-md bg-pri-50 p-6">
              <h3 className="text-sm font-semibold text-primary">Visi</h3>
              <p className="mt-3 text-xl leading-snug font-medium">{visionMission.vision}</p>
            </div>
            <div>
              <h3 className="text-sm font-semibold text-primary">Misi</h3>
              <ol className="mt-3 grid list-decimal gap-3 pl-6 marker:font-bold marker:text-primary">
                {visionMission.missions.map((mission, index) => (
                  // Missions never reorder in place, and two may read the same.
                  // biome-ignore lint/suspicious/noArrayIndexKey: see above
                  <li key={index} className="rounded-md border bg-card px-5 py-4">
                    {mission}
                  </li>
                ))}
              </ol>
            </div>
          </div>
        ) : (
          <p className="text-muted-foreground">Visi dan misi belum tersedia.</p>
        )}
      </section>
      <section id="indeks-desa" aria-labelledby="indeks-desa-title" className="grid gap-6">
        <div>
          <h2 id="indeks-desa-title" className="text-2xl font-semibold tracking-tight md:text-3xl">
            Indeks Desa
          </h2>
          {latest && (
            <p className="mt-1 text-muted-foreground">
              Skor dan status dari kementerian, edisi {editionLabel(latest)}.
            </p>
          )}
        </div>
        {latest ? (
          <div className="grid items-start gap-6 md:grid-cols-2">
            <div className="grid justify-items-start gap-3 rounded-md border bg-card p-6">
              <span className="rounded-md bg-pri-50 px-2 py-0.5 text-xs font-semibold text-primary">
                Desa {statusLabels[latest.status]}
              </span>
              <p className="text-5xl leading-none font-bold tracking-tight tabular-nums">
                {formatScore(latest.score, latest.kind)}{' '}
                <small className="text-base font-medium text-muted-foreground">
                  dari {scales[latest.kind].max}
                </small>
              </p>
              <table className="mt-3 w-full text-left tabular-nums">
                <caption className="sr-only">Riwayat Indeks Desa</caption>
                <thead className="text-sm text-muted-foreground">
                  <tr>
                    <th scope="col" className="py-2 font-medium">
                      Edisi
                    </th>
                    <th scope="col" className="py-2 font-medium">
                      Skor
                    </th>
                    <th scope="col" className="py-2 font-medium">
                      Status
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {editions.map((edition) => (
                    <tr key={edition.id} className="border-t">
                      <td className="py-2">{editionLabel(edition)}</td>
                      <td className="py-2">{formatScore(edition.score, edition.kind)}</td>
                      <td className="py-2">{statusLabels[edition.status]}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
              {editions.some((edition) => edition.kind === 'idm') && (
                <p className="text-sm text-muted-foreground">
                  Edisi sebelum 2024 memakai Indeks Desa Membangun (IDM) dengan skala 0–1.
                </p>
              )}
            </div>
            {scored.length > 0 && (
              <div className="grid gap-5 rounded-md border bg-card p-6">
                <div>
                  <h3 className="text-lg font-semibold">Skor per dimensi</h3>
                  <p className="text-sm text-muted-foreground">Skala 0–100</p>
                </div>
                {scored.map(([key, label]) => {
                  const score = latest[key] ?? 0;
                  return (
                    <div key={key} className="grid gap-2">
                      <div className="flex items-end justify-between gap-3 font-medium">
                        <span>{label}</span>
                        <b className="text-primary tabular-nums">
                          {formatScore(score, 'indeks_desa')}
                        </b>
                      </div>
                      <div className="h-2 overflow-hidden rounded-md bg-muted" aria-hidden="true">
                        <div
                          className="h-full rounded-md bg-primary"
                          style={{ width: `${score}%` }}
                        />
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        ) : (
          <p className="text-muted-foreground">Indeks Desa belum tersedia.</p>
        )}
      </section>
    </main>
  );
}
