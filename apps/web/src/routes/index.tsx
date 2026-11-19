import { createFileRoute } from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import { getHealth, type Health } from '../api/generated';

// Server-only so the browser never needs to reach the API directly.
const fetchHealth = createServerFn({ method: 'GET' }).handler(async (): Promise<Health> => {
  try {
    return (await getHealth()).data;
  } catch {
    return { status: 'unavailable' };
  }
});

export const Route = createFileRoute('/')({
  loader: () => fetchHealth(),
  component: HomePage,
});

function HomePage() {
  const health = Route.useLoaderData();
  return (
    <main>
      <h1>Pranata</h1>
      <p>Status API: {health.status === 'ok' ? 'sehat' : 'tidak tersedia'}</p>
    </main>
  );
}
