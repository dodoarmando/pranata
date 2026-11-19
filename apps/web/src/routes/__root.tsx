import { createRootRoute, HeadContent, Outlet, Scripts } from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import { getVillage, type Village } from '../api/generated';
import '../styles.css';

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

function RootComponent() {
  return (
    <html lang="id">
      <head>
        <HeadContent />
      </head>
      <body>
        <Outlet />
        <Scripts />
      </body>
    </html>
  );
}
