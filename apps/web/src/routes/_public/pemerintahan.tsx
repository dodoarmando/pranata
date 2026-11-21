import { createFileRoute, getRouteApi } from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import {
  type Government,
  getGovernment,
  type Institution,
  listInstitutions,
} from '../../api/generated';
import { holderTitle } from '../../government';
import { institutionTitle } from '../../institution';
import { pageHead } from '../../seo';
import { capitalized, villageTitle } from '../../village';

// Server-only; null before the Village Code is set, or when the API cannot be reached.
const fetchGovernment = createServerFn({ method: 'GET' }).handler(
  async (): Promise<Government | null> => {
    try {
      const response = await getGovernment();
      return response.status === 200 ? response.data : null;
    } catch {
      return null;
    }
  },
);

// Server-only; empty when the API cannot be reached.
const fetchInstitutions = createServerFn({ method: 'GET' }).handler(
  async (): Promise<Institution[]> => {
    try {
      const response = await listInstitutions();
      return response.status === 200 ? response.data : [];
    } catch {
      return [];
    }
  },
);

const root = getRouteApi('__root__');

export const Route = createFileRoute('/_public/pemerintahan')({
  loader: async () => {
    const [government, institutions] = await Promise.all([fetchGovernment(), fetchInstitutions()]);
    return { government, institutions };
  },
  head: (context) => pageHead(context, { title: 'Pemerintahan' }),
  component: GovernmentPage,
});

function GovernmentPage() {
  const village = root.useLoaderData();
  const { government, institutions } = Route.useLoaderData();
  const label = capitalized(government?.hamletLabel ?? 'dusun');
  return (
    <main className="site-container grid gap-12 pt-10 pb-sec md:pt-16">
      <h1 className="-mb-6 text-[2rem] leading-tight font-semibold tracking-tight md:text-[2.5rem]">
        {village ? `Pemerintahan ${villageTitle(village)}` : 'Pemerintahan'}
      </h1>
      <section aria-labelledby="officials-title" className="grid gap-6">
        <h2 id="officials-title" className="text-2xl font-semibold tracking-tight md:text-3xl">
          Perangkat desa
        </h2>
        {government?.officials.length ? (
          <ul className="grid grid-cols-2 gap-4 sm:grid-cols-3 lg:grid-cols-4">
            {government.officials.map((official) => (
              <li
                key={`${official.position}-${official.name}`}
                className="overflow-hidden rounded-md border bg-card"
              >
                {official.photo ? (
                  <img
                    src={`/media/${official.photo}/480`}
                    alt={official.photoAlt ?? ''}
                    width={480}
                    height={600}
                    sizes="(min-width: 1024px) 18rem, (min-width: 640px) 33vw, 50vw"
                    loading="lazy"
                    className="aspect-4/5 w-full object-cover"
                  />
                ) : (
                  <span
                    className="grid aspect-4/5 w-full place-items-center bg-pri-50 text-4xl font-semibold text-primary"
                    aria-hidden="true"
                  >
                    {official.name.charAt(0)}
                  </span>
                )}
                <div className="grid gap-0.5 px-4 py-3">
                  <strong className="leading-snug">{official.name}</strong>
                  <span className="text-sm text-muted-foreground">
                    {holderTitle(official.kind, official.position)}
                  </span>
                </div>
              </li>
            ))}
          </ul>
        ) : (
          <p className="text-muted-foreground">Data perangkat desa belum tersedia.</p>
        )}
      </section>
      <section aria-labelledby="hamlets-title" className="grid gap-6">
        <h2 id="hamlets-title" className="text-2xl font-semibold tracking-tight md:text-3xl">
          {label}
        </h2>
        {government?.hamlets.length ? (
          <dl className="grid max-w-xl rounded-md border bg-card px-5 py-2">
            {government.hamlets.map((hamlet) => (
              <div
                key={hamlet.id}
                className="flex justify-between gap-3 border-b py-3 last:border-b-0"
              >
                <dt className="font-semibold">
                  {label} {hamlet.name}
                </dt>
                <dd className="text-right text-muted-foreground">
                  {hamlet.heads.length ? hamlet.heads.join(', ') : 'Kepala belum ditetapkan'}
                </dd>
              </div>
            ))}
          </dl>
        ) : (
          <p className="text-muted-foreground">Data {label.toLowerCase()} belum tersedia.</p>
        )}
      </section>
      <section id="lembaga" aria-labelledby="institutions-title" className="grid gap-6">
        <h2 id="institutions-title" className="text-2xl font-semibold tracking-tight md:text-3xl">
          Lembaga desa
        </h2>
        {institutions.length ? (
          <ul className="grid gap-4 md:grid-cols-2">
            {institutions.map((institution) => (
              <li
                key={institution.id}
                className="grid content-start gap-3 rounded-md border bg-card p-5"
              >
                <div className="flex items-center gap-3">
                  {institution.logo && (
                    <img
                      src={`/media/${institution.logo}/480`}
                      alt={`Logo ${institution.name}`}
                      width={48}
                      height={48}
                      sizes="3rem"
                      loading="lazy"
                      className="size-12 object-contain"
                    />
                  )}
                  <h3 className="text-lg leading-snug font-semibold">
                    {institutionTitle(institution)}
                  </h3>
                </div>
                <p className="text-muted-foreground">{institution.description}</p>
                {institution.members.length > 0 && (
                  <details>
                    <summary className="cursor-pointer font-medium text-primary">
                      {institution.members.length} pengurus/anggota
                    </summary>
                    <ul className="mt-2 grid">
                      {institution.members.map((member, index) => (
                        <li
                          // Members have no id; their place in the list is who they are.
                          // biome-ignore lint/suspicious/noArrayIndexKey: see above
                          key={index}
                          className="flex justify-between gap-3 border-b py-2 last:border-b-0"
                        >
                          <span>{member.name}</span>
                          <span className="text-right text-muted-foreground">{member.role}</span>
                        </li>
                      ))}
                    </ul>
                  </details>
                )}
              </li>
            ))}
          </ul>
        ) : (
          <p className="text-muted-foreground">Data lembaga desa belum tersedia.</p>
        )}
      </section>
    </main>
  );
}
