import { createFileRoute } from '@tanstack/react-router';
import { publicOrigin } from '../api/fetch';
import { robotsTxt } from '../feeds';

/** What crawlers may visit, and where the sitemap is. */
export const Route = createFileRoute('/robots.txt')({
  server: {
    handlers: {
      GET: () =>
        new Response(robotsTxt(publicOrigin), {
          headers: { 'content-type': 'text/plain; charset=utf-8' },
        }),
    },
  },
});
