import { createRouter } from '@tanstack/react-router';
import { createIsomorphicFn } from '@tanstack/react-start';
import { routeTree } from './routeTree.gen';

// A fresh CSP nonce for every server render, which the framework puts on its inline scripts.
const ssrOptions = createIsomorphicFn()
  .server(() => {
    const bytes = crypto.getRandomValues(new Uint8Array(16));
    return { nonce: btoa(String.fromCharCode(...bytes)) };
  })
  .client(() => ({}));

export function getRouter() {
  return createRouter({ routeTree, scrollRestoration: true, ssr: ssrOptions() });
}
