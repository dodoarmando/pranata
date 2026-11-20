import { createFileRoute } from '@tanstack/react-router';
import { publicOrigin } from '../api/fetch';
import { getSitemap } from '../api/generated';
import { sitemapXml } from '../feeds';

/** The sitemap search engines read, built from the API's Published items. */
export const Route = createFileRoute('/sitemap.xml')({
  server: {
    handlers: {
      GET: async () => {
        // An error, not an empty sitemap, so crawlers keep the URLs they know.
        const response = await getSitemap().catch(() => null);
        if (response?.status !== 200) return new Response(null, { status: 503 });
        return new Response(sitemapXml(publicOrigin, response.data), {
          headers: { 'content-type': 'application/xml; charset=utf-8' },
        });
      },
    },
  },
});
