import { createRootRoute, HeadContent, Outlet, Scripts } from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import { getVillage, type Village } from '../api/generated';

// Server-only, like every API call; null until the vendor sets the Village Code.
const fetchVillage = createServerFn({ method: 'GET' }).handler(
  async (): Promise<Village | null> => {
    try {
      const response = await getVillage();
      return response.status === 200 ? response.data : null;
    } catch {
      return null;
    }
  },
);

export const Route = createRootRoute({
  head: () => ({
    meta: [
      { charSet: 'utf-8' },
      { name: 'viewport', content: 'width=device-width, initial-scale=1' },
      { title: 'Pranata' },
    ],
  }),
  loader: () => fetchVillage(),
  component: RootComponent,
});

// Desa adat names already start with "Desa Adat".
const villageTitle = (village: Village) =>
  village.name.startsWith('Desa Adat ') ? village.name : `Desa ${village.name}`;

const regencyShort = (village: Village) => village.regency.replace(/^Kabupaten /, 'Kab. ');

function RootComponent() {
  const village = Route.useLoaderData();
  return (
    <html lang="id">
      <head>
        <HeadContent />
      </head>
      <body>
        {village && (
          <header>
            <strong>{villageTitle(village)}</strong>
            <small>
              {regencyShort(village)}, {village.province}
            </small>
          </header>
        )}
        <Outlet />
        <footer>
          {village && (
            <>
              <strong>Pemerintah {villageTitle(village)}</strong>
              <p>
                Kec. {village.district}, {regencyShort(village)}, {village.province}
              </p>
            </>
          )}
          <p>Dikelola dengan Pranata</p>
        </footer>
        <Scripts />
      </body>
    </html>
  );
}
