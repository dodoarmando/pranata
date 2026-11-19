import { createFileRoute, getRouteApi, Link } from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import {
  type AuditEntry,
  type ListAuditEntriesParams,
  listAuditEntries,
} from '../../api/generated';
import { ianaZones } from '../../village';

const filterKeys = ['objectType', 'objectId', 'username', 'from', 'to', 'before'] as const;

// Drops what a submitted filter form leaves empty; the router reads `objectId=7` as a number.
// Also the server function's validator, as the router merges the raw query back into the search.
const filterOf = (search: Record<string, unknown>): ListAuditEntriesParams =>
  Object.fromEntries(
    filterKeys.flatMap((key) => {
      const value = search[key];
      return value === undefined || value === '' ? [] : [[key, String(value)]];
    }),
  );

const shown = (value: unknown) =>
  value === null ? '(kosong)' : typeof value === 'string' ? value : JSON.stringify(value);

// Server-only; the page with its details and diff values as text, or the status that refused it.
const fetchAuditPage = createServerFn({ method: 'GET' })
  .validator(filterOf)
  .handler(async ({ data }) => {
    const response = await listAuditEntries(data);
    if (response.status !== 200) return response.status;
    const { entries, next } = response.data;
    return {
      next: next ?? null,
      entries: entries.map(({ details, changes, ...entry }) => ({
        ...entry,
        details: JSON.stringify(details) === '{}' ? '' : JSON.stringify(details),
        changes: changes.map(({ field, from, to }) => ({
          field,
          from: shown(from),
          to: shown(to),
        })),
      })),
    };
  });

const root = getRouteApi('__root__');

export const Route = createFileRoute('/admin/log-audit')({
  validateSearch: filterOf,
  loaderDeps: ({ search }) => search,
  loader: ({ deps }) => fetchAuditPage({ data: deps }),
  head: () => ({ meta: [{ title: 'Log audit · Pranata' }] }),
  component: AuditLog,
});

const actor = (entry: Pick<AuditEntry, 'actor' | 'username'>) =>
  entry.username ?? (entry.actor === 'system' ? 'Sistem' : 'Tidak dikenal');

function AuditLog() {
  const village = root.useLoaderData();
  const page = Route.useLoaderData();
  const filter = Route.useSearch();
  const { before, ...newest } = filter;
  const zone = village?.timeZone ?? 'WIB';
  const time = new Intl.DateTimeFormat('id-ID', {
    dateStyle: 'medium',
    timeStyle: 'medium',
    timeZone: ianaZones[zone],
  });

  return (
    <>
      <h1>Log audit</h1>
      <form method="get">
        <label>
          Jenis objek
          <input name="objectType" defaultValue={filter.objectType} />
        </label>
        <label>
          ID objek
          <input name="objectId" defaultValue={filter.objectId} />
        </label>
        <label>
          Pengguna
          <input name="username" autoCapitalize="none" defaultValue={filter.username} />
        </label>
        <label>
          Dari tanggal
          <input name="from" type="date" defaultValue={filter.from} />
        </label>
        <label>
          Sampai tanggal
          <input name="to" type="date" defaultValue={filter.to} />
        </label>
        <button type="submit">Saring</button> <Link to="/admin/log-audit">Hapus saringan</Link>
      </form>
      {page === 403 ? (
        <p role="alert">Anda tidak memiliki izin membaca log audit.</p>
      ) : typeof page === 'number' ? (
        <p role="alert">Saringan tidak valid.</p>
      ) : page.entries.length === 0 ? (
        <p>Tidak ada entri yang cocok.</p>
      ) : (
        <>
          <table>
            <thead>
              <tr>
                <th>Waktu ({zone})</th>
                <th>Pelaku</th>
                <th>Tindakan</th>
                <th>Objek</th>
                <th>Perubahan</th>
                <th>Keterangan</th>
              </tr>
            </thead>
            <tbody>
              {page.entries.map((entry) => (
                <tr key={entry.id}>
                  <td>{time.format(new Date(entry.at))}</td>
                  <td>{actor(entry)}</td>
                  <td>{entry.action}</td>
                  <td>
                    {entry.objectType} {entry.objectId}
                  </td>
                  <td>
                    <ul>
                      {entry.changes.map((change) => (
                        <li key={change.field}>
                          {change.field}: {change.from} → {change.to}
                        </li>
                      ))}
                    </ul>
                  </td>
                  <td>{entry.details}</td>
                </tr>
              ))}
            </tbody>
          </table>
          <nav aria-label="Halaman log audit">
            {before && (
              <Link to="/admin/log-audit" search={newest}>
                Terbaru
              </Link>
            )}{' '}
            {page.next && (
              <Link to="/admin/log-audit" search={{ ...filter, before: page.next }}>
                Lebih lama
              </Link>
            )}
          </nav>
        </>
      )}
    </>
  );
}
