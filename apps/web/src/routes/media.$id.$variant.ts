import { createFileRoute } from '@tanstack/react-router';
import { baseUrl } from '../api/fetch';

// The API's response headers that reach the browser: the type and the immutable caching.
const passed = ['content-type', 'cache-control', 'x-content-type-options'];

/** Public images, which the browser fetches here since it never reaches the API. */
export const Route = createFileRoute('/media/$id/$variant')({
  server: {
    handlers: {
      GET: async ({ params }) => {
        const path = [params.id, params.variant].map(encodeURIComponent).join('/');
        const response = await fetch(`${baseUrl}/media/${path}`);
        const headers = new Headers();
        for (const name of passed) {
          const value = response.headers.get(name);
          if (value) headers.set(name, value);
        }
        return new Response(response.ok ? response.body : null, {
          status: response.status,
          headers,
        });
      },
    },
  },
});
