import { createFileRoute } from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import { getHealth, type Health } from '../../api/generated';

// Server-only so the browser never needs to reach the API directly.
const fetchHealth = createServerFn({ method: 'GET' }).handler(async (): Promise<Health> => {
  try {
    return (await getHealth()).data;
  } catch {
    return { status: 'unavailable' };
  }
});

export const Route = createFileRoute('/_public/')({
  loader: () => fetchHealth(),
  component: HomePage,
});

function HomePage() {
  const health = Route.useLoaderData();
  return (
    <main className="site-container grid gap-3 pt-10 pb-sec md:pt-16">
      <h1 className="text-[2rem] leading-tight font-semibold tracking-tight md:text-[2.5rem] xl:text-5xl">
        Pranata
      </h1>
      <p className="text-muted-foreground md:text-lg">
        Status API: {health.status === 'ok' ? 'sehat' : 'tidak tersedia'}
      </p>
    </main>
  );
}
