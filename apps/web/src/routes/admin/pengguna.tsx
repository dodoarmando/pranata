import { createFileRoute, useRouter } from '@tanstack/react-router';
import { createServerFn, useServerFn } from '@tanstack/react-start';
import { type FormEvent, useState } from 'react';
import {
  createAccessLink,
  createUser,
  listRoles,
  listUsers,
  type NewUser,
  revokeSessions,
  type UserChange,
  updateUser,
} from '../../api/generated';
import { useSudo } from '../../sudo';

// Server-only; the Users and Roles, or the status that refused them.
const fetchUsers = createServerFn({ method: 'GET' }).handler(async () => {
  const [users, roles] = await Promise.all([listUsers(), listRoles()]);
  if (users.status !== 200) return users.status;
  if (roles.status !== 200) return roles.status;
  return { users: users.data, roles: roles.data };
});

const createUserFn = createServerFn({ method: 'POST' })
  .validator((data: NewUser) => data)
  .handler(async ({ data }) => {
    const response = await createUser(data);
    return response.status === 201
      ? { status: response.status, token: response.data.token }
      : { status: response.status };
  });

const resetFn = createServerFn({ method: 'POST' })
  .validator((id: string) => id)
  .handler(async ({ data }) => {
    const response = await createAccessLink(data);
    return response.status === 201
      ? { status: response.status, token: response.data.token }
      : { status: response.status };
  });

const updateUserFn = createServerFn({ method: 'POST' })
  .validator((data: { id: string; change: UserChange }) => data)
  .handler(async ({ data }) => (await updateUser(data.id, data.change)).status);

const revokeSessionsFn = createServerFn({ method: 'POST' })
  .validator((id: string) => id)
  .handler(async ({ data }) => (await revokeSessions(data)).status);

export const Route = createFileRoute('/admin/pengguna')({
  loader: () => fetchUsers(),
  head: () => ({ meta: [{ title: 'Pengguna · Pranata' }] }),
  component: Users,
});

const createErrors: Record<number, string> = {
  409: 'Nama pengguna itu sudah dipakai. Nama pengguna tidak pernah dipakai ulang.',
  422: 'Nama pengguna 3–32 karakter: huruf kecil, angka, titik, garis bawah, atau tanda hubung.',
};

const updateErrors: Record<number, string> = {
  409: 'Harus tetap ada setidaknya satu Administrator aktif.',
};

function Users() {
  const page = Route.useLoaderData();
  const router = useRouter();
  const sudo = useSudo();
  const submitCreate = useServerFn(createUserFn);
  const submitReset = useServerFn(resetFn);
  const submitUpdate = useServerFn(updateUserFn);
  const submitRevoke = useServerFn(revokeSessionsFn);
  const [link, setLink] = useState<{ username: string; url: string } | null>(null);
  const [message, setMessage] = useState('');

  if (typeof page === 'number') {
    return (
      <>
        <h1>Pengguna</h1>
        <p role="alert">Hanya Administrator yang dapat mengelola pengguna.</p>
      </>
    );
  }

  /** Runs an action in sudo mode, then shows its error or reloads the list. */
  const act =
    (errors: Record<number, string>, action: () => Promise<number>, done = '') =>
    () =>
      sudo.run(async () => {
        const status = await action();
        if (status === 428) return status;
        setMessage(errors[status] ?? (status < 300 ? done : 'Tindakan gagal. Coba lagi.'));
        if (status < 300) await router.invalidate();
        return status;
      });

  const showLink = (username: string, token: string) =>
    setLink({ username, url: `${window.location.origin}/akses#${token}` });

  function onCreate(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const formElement = event.currentTarget;
    const form = new FormData(formElement);
    const user = { username: String(form.get('username')), roleId: String(form.get('roleId')) };
    return act(createErrors, async () => {
      const result = await submitCreate({ data: user });
      if (result.token) {
        showLink(user.username, result.token);
        formElement.reset();
      }
      return result.status;
    })();
  }

  return (
    <>
      <h1>Pengguna</h1>
      {sudo.prompt}
      {message && <p role="alert">{message}</p>}
      {link && (
        <section aria-labelledby="link-title">
          <h2 id="link-title">Tautan Akses untuk {link.username}</h2>
          <input readOnly value={link.url} aria-label="Tautan Akses" />
          <p>
            Berikan tautan ini langsung kepada pengguna. Tautan berlaku 24 jam, hanya sekali pakai,
            dan tidak ditampilkan lagi.
          </p>
        </section>
      )}
      <form onSubmit={onCreate} aria-labelledby="new-user-title">
        <h2 id="new-user-title">Pengguna baru</h2>
        <label>
          Nama pengguna
          <input name="username" autoCapitalize="none" required />
        </label>
        <label>
          Peran
          <select name="roleId" required>
            {page.roles.map((role) => (
              <option key={role.id} value={role.id}>
                {role.name}
              </option>
            ))}
          </select>
        </label>
        <button type="submit">Buat pengguna</button>
      </form>
      <table>
        <thead>
          <tr>
            <th>Nama pengguna</th>
            <th>Peran</th>
            <th>Status</th>
            <th>Tindakan</th>
          </tr>
        </thead>
        <tbody>
          {page.users.map((user) => (
            <tr key={user.id}>
              <td>{user.username}</td>
              <td>
                <select
                  aria-label={`Peran ${user.username}`}
                  value={user.roleId}
                  onChange={(event) => {
                    const change = { roleId: event.target.value };
                    act(updateErrors, () => submitUpdate({ data: { id: user.id, change } }))();
                  }}
                >
                  {page.roles.map((role) => (
                    <option key={role.id} value={role.id}>
                      {role.name}
                    </option>
                  ))}
                </select>
              </td>
              <td>{user.active ? 'Aktif' : 'Nonaktif'}</td>
              <td>
                {user.active && (
                  <>
                    <button
                      type="button"
                      onClick={act({}, async () => {
                        const result = await submitReset({ data: user.id });
                        if (result.token) showLink(user.username, result.token);
                        return result.status;
                      })}
                    >
                      Buat Tautan Akses
                    </button>{' '}
                  </>
                )}
                <button
                  type="button"
                  onClick={act(
                    {},
                    () => submitRevoke({ data: user.id }),
                    `Semua sesi ${user.username} telah dicabut.`,
                  )}
                >
                  Cabut semua sesi
                </button>{' '}
                {user.active ? (
                  <button
                    type="button"
                    onClick={() =>
                      window.confirm(
                        `Nonaktifkan ${user.username}? Semua sesinya langsung berakhir.`,
                      ) &&
                      act(updateErrors, () =>
                        submitUpdate({ data: { id: user.id, change: { active: false } } }),
                      )()
                    }
                  >
                    Nonaktifkan
                  </button>
                ) : (
                  <button
                    type="button"
                    onClick={act(updateErrors, () =>
                      submitUpdate({ data: { id: user.id, change: { active: true } } }),
                    )}
                  >
                    Aktifkan
                  </button>
                )}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </>
  );
}
