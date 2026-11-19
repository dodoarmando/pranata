import {
  createFileRoute,
  getRouteApi,
  Link,
  Outlet,
  redirect,
  useNavigate,
} from '@tanstack/react-router';
import { createServerFn, useServerFn } from '@tanstack/react-start';
import { getMe, type Me, signOut } from '../../api/generated';
import { villageTitle } from '../../village';

// Server-only: forwards the session cookie; null without a live session.
const fetchMe = createServerFn({ method: 'GET' }).handler(async (): Promise<Me | null> => {
  const response = await getMe();
  return response.status === 200 ? response.data : null;
});

const signOutFn = createServerFn({ method: 'POST' }).handler(async () => {
  await signOut();
});

// Module navigation from the prototype; the modules arrive with their own tickets.
const modules: [group: string, items: [label: string, slug: string][]][] = [
  [
    'Desa',
    [
      ['Profil desa', 'desa'],
      ['Wilayah', 'wilayah'],
      ['Perangkat desa', 'perangkat-desa'],
      ['Lembaga desa', 'lembaga-desa'],
      ['Indeks Desa', 'indeks-desa'],
    ],
  ],
  [
    'Kependudukan',
    [
      ['Penduduk', 'penduduk'],
      ['Keluarga', 'keluarga'],
      ['Peristiwa', 'peristiwa'],
      ['Rekap bulanan', 'rekap-bulanan'],
      ['Buku induk', 'buku-induk'],
      ['Buku mutasi', 'buku-mutasi'],
    ],
  ],
  [
    'Surat',
    [
      ['Surat layanan', 'surat'],
      ['Surat tercatat', 'surat-tercatat'],
      ['Buku agenda surat keluar', 'buku-agenda'],
      ['Jenis surat', 'jenis-surat'],
      ['Penomoran', 'penomoran'],
    ],
  ],
  [
    'Situs web',
    [
      ['Hero beranda', 'hero-beranda'],
      ['Sambutan Kepala Desa', 'sambutan'],
      ['Berita', 'berita'],
      ['Pengumuman', 'pengumuman'],
      ['Agenda', 'agenda'],
      ['Halaman', 'halaman'],
      ['Galeri', 'galeri'],
      ['Kontak penting', 'kontak-penting'],
      ['Mitra', 'mitra'],
      ['Dokumen transparansi', 'dokumen'],
      ['Ringkasan APBDes', 'apbdes'],
    ],
  ],
  [
    'Sistem',
    [
      ['Pengguna', 'pengguna'],
      ['Peran', 'peran'],
      ['Log audit', 'log-audit'],
    ],
  ],
];

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

  async function onSignOut() {
    await submitSignOut();
    await navigate({ to: '/masuk' });
  }

  return (
    <>
      <aside>
        <nav aria-label="Menu admin">
          <Link to="/admin">
            <strong>{village ? villageTitle(village) : 'Pranata'}</strong> <small>Admin desa</small>
          </Link>
          <Link to="/admin" activeOptions={{ exact: true }}>
            Beranda
          </Link>
          {modules.map(([group, items]) => (
            <details key={group}>
              <summary>{group}</summary>
              <ul>
                {items.map(([label, slug]) => (
                  <li key={slug}>
                    <a href={`/admin/${slug}`}>{label}</a>
                  </li>
                ))}
              </ul>
            </details>
          ))}
        </nav>
        <p>
          <strong>{me.username}</strong> <small>{me.role}</small>
        </p>
        <Link to="/admin/akun">Akun saya</Link>
        <button type="button" onClick={onSignOut}>
          Keluar
        </button>
      </aside>
      <main>
        <Outlet />
      </main>
    </>
  );
}
