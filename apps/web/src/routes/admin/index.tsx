import { createFileRoute } from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import { getDashboard } from '../../api/generated';
import { PageHead } from '../../components/admin';
import { Card } from '../../components/ui/card';

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
      <PageHead title="Beranda" />
      {dashboard && (
        <div className="grid grid-cols-2 gap-4 xl:grid-cols-4">
          <Card role="region" aria-labelledby="active-users" className="gap-1 px-5 py-5">
            <h2 id="active-users" className="text-muted-foreground">
              Pengguna aktif
            </h2>
            <p className="text-[1.625rem] leading-tight font-semibold tracking-tight">
              {dashboard.activeUsers}
            </p>
          </Card>
        </div>
      )}
    </>
  );
}
