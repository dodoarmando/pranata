import { createFileRoute } from '@tanstack/react-router';
import { publicOrigin } from '../api/fetch';
import { getSitemap } from '../api/generated';
import { sitemapXml } from '../feeds';

export const Route = createFileRoute('/sitemap.xml')({
  server: {
    handlers: {
      GET: async () => {
        const response = await getSitemap();
        // An error, not an empty sitemap, so crawlers keep the URLs they know.
        if (response.status !== 200) return new Response(null, { status: 503 });
        return new Response(sitemapXml(publicOrigin, response.data), {
          headers: { 'content-type': 'application/xml; charset=utf-8' },
        });
      },
    },
  },
});
