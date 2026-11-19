import { createFileRoute } from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import { getDashboard } from '../../api/generated';

// Server-only; null when the session ended since the shell checked it.
const fetchDashboard = createServerFn({ method: 'GET' }).handler(async () => {
  const response = await getDashboard();
  return response.status === 200 ? response.data : null;
});

export const Route = createFileRoute('/admin/')({
  loader: () => fetchDashboard(),
  component: Dashboard,
});

function Dashboard() {
  const dashboard = Route.useLoaderData();
  return (
    <>
      <h1>Beranda</h1>
      {dashboard && (
        <section aria-labelledby="active-users">
          <h2 id="active-users">Pengguna aktif</h2>
          <p>{dashboard.activeUsers}</p>
        </section>
      )}
    </>
  );
}
