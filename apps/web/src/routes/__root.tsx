import geist from '@fontsource-variable/geist/files/geist-latin-wght-normal.woff2?url';
import { createRootRoute, HeadContent, Outlet, Scripts } from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import { applySavedSettings } from '../accessibility';
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

// The head script's CSP source; computed once, as the script never changes at runtime.
let scriptHash: Promise<string> | undefined;
const hashOf = async (script: string) => {
  const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(script));
  return `'sha256-${btoa(String.fromCharCode(...new Uint8Array(digest)))}'`;
};

export const Route = createRootRoute({
  // Scripts run only from this origin, the framework's own inline ones by the request's
  // nonce, and the accessibility head script by its hash.
  headers: async ({ ssr }) => {
    if (!ssr?.nonce) return;
    scriptHash ??= hashOf(applySavedSettings);
    return {
      'Content-Security-Policy': `script-src 'self' 'nonce-${ssr.nonce}' ${await scriptHash}; object-src 'none'; base-uri 'none'`,
    };
  },
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
    // The head script sets the saved accessibility classes and font size before hydration.
    <html lang="id" suppressHydrationWarning>
      <head>
        {/* biome-ignore lint/security/noDangerouslySetInnerHtml: a constant, allowed by its CSP hash */}
        <script dangerouslySetInnerHTML={{ __html: applySavedSettings }} />
        <HeadContent />
      </head>
      <body>
        <Outlet />
        <Scripts />
      </body>
    </html>
  );
}
