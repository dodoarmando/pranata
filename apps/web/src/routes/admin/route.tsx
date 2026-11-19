import {
  createFileRoute,
  getRouteApi,
  Link,
  Outlet,
  redirect,
  useLocation,
  useNavigate,
} from '@tanstack/react-router';
import { createServerFn, useServerFn } from '@tanstack/react-start';
import {
  ChevronRight,
  House,
  Landmark,
  LogOut,
  MapPinned,
  PanelLeft,
  ScrollText,
  ShieldCheck,
  UserRound,
  Users,
} from 'lucide-react';
import { Fragment, useState } from 'react';
import { getMe, type Me, signOut } from '../../api/generated';
import { Button } from '../../components/ui/button';
import { Sheet, SheetContent, SheetTitle, SheetTrigger } from '../../components/ui/sheet';
import { VillageLogo } from '../../components/village-logo';
import { villageTitle } from '../../village';

// Server-only: forwards the session cookie; null without a live session.
const fetchMe = createServerFn({ method: 'GET' }).handler(async (): Promise<Me | null> => {
  const response = await getMe();
  return response.status === 200 ? response.data : null;
});

const signOutFn = createServerFn({ method: 'POST' }).handler(async () => {
  await signOut();
});

// Only pages that exist; each module adds its own entries.
const pages = [
  { group: '', to: '/admin', label: 'Beranda', icon: House },
  { group: 'Desa', to: '/admin/profil-desa', label: 'Profil desa', icon: Landmark },
  { group: 'Desa', to: '/admin/wilayah', label: 'Wilayah', icon: MapPinned },
  { group: 'Sistem', to: '/admin/pengguna', label: 'Pengguna', icon: Users },
  { group: 'Sistem', to: '/admin/peran', label: 'Peran', icon: ShieldCheck },
  { group: 'Sistem', to: '/admin/log-audit', label: 'Log audit', icon: ScrollText },
] as const;

const navItemClass =
  'flex h-8 items-center gap-2 rounded-md px-2 hover:bg-accent data-[status=active]:bg-accent data-[status=active]:font-medium [&_svg]:size-4';

const root = getRouteApi('__root__');

export const Route = createFileRoute('/admin')({
  beforeLoad: async () => {
    const me = await fetchMe();
    if (!me) throw redirect({ to: '/masuk' });
    return { me };
  },
  head: () => ({ meta: [{ title: 'Admin desa · Pranata' }] }),
  component: AdminShell,
});

function AdminShell() {
  const village = root.useLoaderData();
  const { me } = Route.useRouteContext();
  const navigate = useNavigate();
  const submitSignOut = useServerFn(signOutFn);

  const [menuOpen, setMenuOpen] = useState(false);
  const pathname = useLocation({ select: (location) => location.pathname });
  const page = pages.find((entry) => entry.to === pathname.replace(/\/$/, ''));
  const crumbs = pathname.startsWith('/admin/akun')
    ? ['Akun saya']
    : page?.group
      ? [page.group, page.label]
      : ['Beranda'];

  async function onSignOut() {
    await submitSignOut();
    await navigate({ to: '/masuk' });
  }

  const close = () => setMenuOpen(false);
  const sidebar = (
    <nav aria-label="Menu admin" className="flex flex-1 flex-col gap-2">
      <Link
        to="/admin"
        onClick={close}
        className="flex items-center gap-2 rounded-lg p-2 hover:bg-accent"
      >
        <VillageLogo
          village={village}
          className="size-8"
          placeholderClassName="rounded-lg font-semibold"
        />
        <span className="grid min-w-0 leading-tight">
          <strong className="truncate">{village ? villageTitle(village) : 'Pranata'}</strong>
          <small className="text-xs text-muted-foreground">Admin desa</small>
        </span>
      </Link>
      <div className="grid gap-px">
        {pages.map((entry, index) => (
          <Fragment key={entry.to}>
            {entry.group && pages[index - 1]?.group !== entry.group && (
              <p className="px-2 pt-3 pb-1.5 text-xs font-medium text-muted-foreground">
                {entry.group}
              </p>
            )}
            <Link
              to={entry.to}
              activeOptions={{ exact: true }}
              onClick={close}
              className={navItemClass}
            >
              <entry.icon aria-hidden="true" />
              {entry.label}
            </Link>
          </Fragment>
        ))}
      </div>
      <div className="mt-auto grid gap-px border-t pt-2">
        <p className="flex items-center gap-2 p-2">
          <span
            className="grid size-8 flex-none place-items-center rounded-lg bg-border text-xs font-medium uppercase"
            aria-hidden="true"
          >
            {me.username.slice(0, 2)}
          </span>
          <span className="grid min-w-0 leading-tight">
            <strong className="truncate font-medium">{me.username}</strong>
            <small className="truncate text-xs text-muted-foreground">{me.role}</small>
          </span>
        </p>
        <Link to="/admin/akun" onClick={close} className={navItemClass}>
          <UserRound aria-hidden="true" />
          Akun saya
        </Link>
        <button type="button" onClick={onSignOut} className={`${navItemClass} text-left`}>
          <LogOut aria-hidden="true" />
          Keluar
        </button>
      </div>
    </nav>
  );

  return (
    <div className="flex min-h-dvh bg-muted">
      <aside className="sticky top-0 hidden h-dvh w-64 flex-none flex-col overflow-y-auto p-2 lg:flex">
        {sidebar}
      </aside>
      <div className="flex min-w-0 flex-1 flex-col bg-background lg:my-2 lg:mr-2 lg:rounded-xl lg:border lg:shadow-xs">
        <header className="sticky top-0 z-10 flex h-14 flex-none items-center gap-2 border-b bg-background px-4 lg:rounded-t-xl">
          <Sheet open={menuOpen} onOpenChange={setMenuOpen}>
            <SheetTrigger
              render={<Button variant="ghost" size="icon-sm" className="lg:hidden" />}
              aria-label="Buka menu"
            >
              <PanelLeft aria-hidden="true" />
            </SheetTrigger>
            <SheetContent side="left" className="w-[min(300px,85vw)] p-2">
              <SheetTitle className="sr-only">Menu admin</SheetTitle>
              {sidebar}
            </SheetContent>
          </Sheet>
          <span className="h-4 w-px bg-border lg:hidden" />
          <p className="flex min-w-0 items-center gap-1.5 pl-1">
            {crumbs.map((crumb, index) => (
              <Fragment key={crumb}>
                {index > 0 && (
                  <ChevronRight className="size-4 text-muted-foreground" aria-hidden="true" />
                )}
                <span className={index < crumbs.length - 1 ? 'text-muted-foreground' : 'truncate'}>
                  {crumb}
                </span>
              </Fragment>
            ))}
          </p>
        </header>
        <main className="flex flex-col gap-5 p-4 sm:p-6">
          <Outlet />
        </main>
      </div>
    </div>
  );
}
