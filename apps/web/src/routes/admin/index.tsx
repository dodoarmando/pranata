import { createFileRoute, getRouteApi, Link } from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import { calendarDate } from '../../announcement';
import { type DraftKind, getDashboard } from '../../api/generated';
import { PageHead } from '../../components/admin';
import { Card } from '../../components/ui/card';
import { ianaZones } from '../../village';

// Server-only; null when the session ended since the shell checked it.
const fetchDashboard = createServerFn({ method: 'GET' }).handler(async () => {
  const response = await getDashboard();
  return response.status === 200 ? response.data : null;
});

const root = getRouteApi('__root__');

const draftKinds = {
  newsPost: { label: 'Berita', to: '/admin/berita/$id' },
  announcement: { label: 'Pengumuman', to: '/admin/pengumuman/$id' },
  event: { label: 'Agenda', to: '/admin/agenda/$id' },
  page: { label: 'Halaman', to: '/admin/halaman/$id' },
} as const satisfies Record<DraftKind, { label: string; to: string }>;

export const Route = createFileRoute('/admin/')({
  loader: () => fetchDashboard(),
  component: Dashboard,
});

function Dashboard() {
  const dashboard = Route.useLoaderData();
  const village = root.useLoaderData();
  const date = new Intl.DateTimeFormat('id-ID', {
    dateStyle: 'medium',
    timeZone: ianaZones[village?.timeZone ?? 'WIB'],
  });
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
          {dashboard.drafts && (
            <Card role="region" aria-labelledby="drafts" className="col-span-2 gap-2 px-5 py-5">
              <h2 id="drafts" className="text-muted-foreground">
                Konten menunggu terbit
              </h2>
              {dashboard.drafts.length ? (
                <ul className="grid gap-1">
                  {dashboard.drafts.map((draft) => (
                    <li key={draft.id} className="flex flex-wrap justify-between gap-x-3">
                      <Link
                        to={draftKinds[draft.kind].to}
                        params={{ id: draft.id }}
                        className="min-w-0 truncate font-medium underline-offset-4 hover:underline"
                      >
                        {draft.title}
                      </Link>
                      <span className="text-sm text-muted-foreground">
                        {draftKinds[draft.kind].label} · {date.format(new Date(draft.updatedAt))}
                      </span>
                    </li>
                  ))}
                </ul>
              ) : (
                <p className="font-medium">Tidak ada draf.</p>
              )}
            </Card>
          )}
          {dashboard.endingAnnouncements && (
            <Card
              role="region"
              aria-labelledby="ending-announcements"
              className="col-span-2 gap-2 px-5 py-5"
            >
              <h2 id="ending-announcements" className="text-muted-foreground">
                Pengumuman segera berakhir
              </h2>
              {dashboard.endingAnnouncements.length ? (
                <ul className="grid gap-1">
                  {dashboard.endingAnnouncements.map((announcement) => (
                    <li key={announcement.id} className="flex flex-wrap justify-between gap-x-3">
                      <Link
                        to="/admin/pengumuman/$id"
                        params={{ id: announcement.id }}
                        className="min-w-0 truncate font-medium underline-offset-4 hover:underline"
                      >
                        {announcement.title}
                      </Link>
                      <span className="text-sm text-muted-foreground">
                        Berakhir {calendarDate(announcement.endDate)}
                      </span>
                    </li>
                  ))}
                </ul>
              ) : (
                <p className="font-medium">Tidak ada yang berakhir dalam 7 hari.</p>
              )}
            </Card>
          )}
        </div>
      )}
    </>
  );
}
