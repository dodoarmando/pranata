import { createFileRoute } from '@tanstack/react-router';
import { publicOrigin } from '../api/fetch';
import { robotsTxt } from '../feeds';

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
