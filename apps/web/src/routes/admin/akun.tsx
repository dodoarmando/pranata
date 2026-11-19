import { createFileRoute, getRouteApi, useRouter } from '@tanstack/react-router';
import { createServerFn, useServerFn } from '@tanstack/react-start';
import { type FormEvent, useState } from 'react';
import { changePassword, listSessions, signOutOthers } from '../../api/generated';
import { describeDevice } from '../../device';
import { passwordHint, passwordProblems } from '../../password';
import { useSudo } from '../../sudo';
import { ianaZones } from '../../village';

// Server-only; null when the session ended since the shell checked it.
const fetchSessions = createServerFn({ method: 'GET' }).handler(async () => {
  const response = await listSessions();
  return response.status === 200 ? response.data : null;
});

const changePasswordFn = createServerFn({ method: 'POST' })
  .validator((password: string) => password)
  .handler(async ({ data }) => {
    const response = await changePassword({ password: data });
    return response.status === 422
      ? { status: response.status, problem: response.data.problem }
      : { status: response.status };
  });

const signOutOthersFn = createServerFn({ method: 'POST' }).handler(
  async () => (await signOutOthers()).status,
);

const root = getRouteApi('__root__');

export const Route = createFileRoute('/admin/akun')({
  loader: () => fetchSessions(),
  head: () => ({ meta: [{ title: 'Akun saya · Pranata' }] }),
  component: OwnUserPage,
});

function OwnUserPage() {
  const village = root.useLoaderData();
  const sessions = Route.useLoaderData() ?? [];
  const { me } = Route.useRouteContext();
  const router = useRouter();
  const sudo = useSudo();
  const submitChange = useServerFn(changePasswordFn);
  const submitSignOutOthers = useServerFn(signOutOthersFn);
  const [message, setMessage] = useState('');
  const time = new Intl.DateTimeFormat('id-ID', {
    dateStyle: 'medium',
    timeStyle: 'short',
    timeZone: ianaZones[village?.timeZone ?? 'WIB'],
  });

  function onChangePassword(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const form = event.currentTarget;
    const password = String(new FormData(form).get('password'));
    return sudo.run(async () => {
      const result = await submitChange({ data: password });
      if (result.status === 428) return result.status;
      if (result.status === 204) form.reset();
      setMessage(
        result.status === 204
          ? 'Kata sandi telah diganti.'
          : result.problem
            ? passwordProblems[result.problem]
            : 'Tindakan gagal. Coba lagi.',
      );
      return result.status;
    });
  }

  async function onSignOutOthers() {
    const status = await submitSignOutOthers();
    setMessage(
      status === 204
        ? 'Anda telah keluar dari semua perangkat lain.'
        : 'Tindakan gagal. Coba lagi.',
    );
    await router.invalidate();
  }

  return (
    <>
      <h1>Akun saya</h1>
      <p>
        <strong>{me.username}</strong> <small>{me.role}</small>
      </p>
      {sudo.prompt}
      {message && <p role="alert">{message}</p>}
      <form onSubmit={onChangePassword} aria-labelledby="password-title">
        <h2 id="password-title">Ganti kata sandi</h2>
        <label>
          Kata sandi baru
          <input
            name="password"
            type="password"
            autoComplete="new-password"
            minLength={12}
            aria-describedby="password-hint"
            required
          />
        </label>
        <p id="password-hint">{passwordHint}</p>
        <button type="submit">Ganti kata sandi</button>
      </form>
      <section aria-labelledby="sessions-title">
        <h2 id="sessions-title">Sesi aktif</h2>
        <table>
          <thead>
            <tr>
              <th>Perangkat</th>
              <th>Alamat IP</th>
              <th>Aktivitas terakhir</th>
            </tr>
          </thead>
          <tbody>
            {sessions.map((session) => (
              <tr key={`${session.lastSeenAt}-${session.userAgent}-${session.ip}`}>
                <td>
                  {describeDevice(session.userAgent)}
                  {session.current && <small> (perangkat ini)</small>}
                </td>
                <td>{session.ip ?? '—'}</td>
                <td>{time.format(new Date(session.lastSeenAt))}</td>
              </tr>
            ))}
          </tbody>
        </table>
        <button type="button" onClick={onSignOutOthers} disabled={sessions.length < 2}>
          Keluar dari perangkat lain
        </button>
      </section>
    </>
  );
}
