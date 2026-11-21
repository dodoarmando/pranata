import geist from '@fontsource-variable/geist/files/geist-latin-wght-normal.woff2?url';
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
    // The same hashed file styles.css names, fetched before the stylesheet asks for it.
    links: [
      { rel: 'preload', href: geist, as: 'font', type: 'font/woff2', crossOrigin: 'anonymous' },
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
