import { createFileRoute, Link } from '@tanstack/react-router';
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
  const missing = dashboard?.missingVillageData;
  const toFill = missing
    ? [
        ...(missing.logo ? [{ label: 'Logo desa', to: '/admin/profil-desa' as const }] : []),
        ...(missing.visionMission
          ? [{ label: 'Visi dan misi', to: '/admin/profil-desa' as const }]
          : []),
        ...missing.unfilledPositions.map((label) => ({
          label,
          to: '/admin/pemerintahan' as const,
        })),
      ]
    : [];
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
          <Card
            role="region"
            aria-labelledby="missing-village-data"
            className="col-span-2 gap-2 px-5 py-5"
          >
            <h2 id="missing-village-data" className="text-muted-foreground">
              Data desa perlu dilengkapi
            </h2>
            {toFill.length ? (
              <ul className="grid gap-1">
                {toFill.map((item) => (
                  <li key={item.label}>
                    <Link to={item.to} className="font-medium underline-offset-4 hover:underline">
                      {item.label}
                    </Link>
                  </li>
                ))}
              </ul>
            ) : (
              <p className="font-medium">Semua sudah lengkap.</p>
            )}
          </Card>
        </div>
      )}
    </>
  );
}
