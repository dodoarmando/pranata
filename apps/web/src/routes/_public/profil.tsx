import { createFileRoute, getRouteApi } from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import { getVisionMission, type Village, type VisionMission } from '../../api/generated';
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

const root = getRouteApi('__root__');

export const Route = createFileRoute('/_public/profil')({
  loader: () => fetchVisionMission(),
  head: ({ matches }) => {
    // The root match comes first; its loader data is typed only through getRouteApi.
    const village = matches[0]?.loaderData as Village | null | undefined;
    return { meta: [{ title: village ? `Profil — ${villageTitle(village)}` : 'Profil' }] };
  },
  component: ProfilePage,
});

function ProfilePage() {
  const village = root.useLoaderData();
  const visionMission = Route.useLoaderData();
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
    </main>
  );
}
