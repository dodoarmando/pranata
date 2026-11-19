import { createFileRoute, getRouteApi, useRouter } from '@tanstack/react-router';
import { createServerFn, useServerFn } from '@tanstack/react-start';
import { type FormEvent, useState } from 'react';
import {
  changePassword,
  createPasskey,
  deletePasskey,
  listPasskeys,
  listSessions,
  type NewPasskey,
  signOutOthers,
  startPasskeyRegistration,
} from '../../api/generated';
import { describeDevice } from '../../device';
import { createPasskeyCredential } from '../../passkey';
import { passwordHint, passwordProblems } from '../../password';
import { useSudo } from '../../sudo';
import { ianaZones } from '../../village';

// Server-only; null when the session ended since the shell checked it.
const fetchOwnUser = createServerFn({ method: 'GET' }).handler(async () => {
  const [sessions, passkeys] = await Promise.all([listSessions(), listPasskeys()]);
  return sessions.status === 200 && passkeys.status === 200
    ? { sessions: sessions.data, passkeys: passkeys.data }
    : null;
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

const startRegistrationFn = createServerFn({ method: 'POST' }).handler(async () => {
  const response = await startPasskeyRegistration();
  if (response.status !== 200) return { status: response.status };
  const { ceremonyId, options } = response.data;
  return { status: response.status, ceremonyId, options: JSON.stringify(options) };
});

const createPasskeyFn = createServerFn({ method: 'POST' })
  .validator((data: NewPasskey) => data)
  .handler(async ({ data }) => (await createPasskey(data)).status);

const deletePasskeyFn = createServerFn({ method: 'POST' })
  .validator((id: string) => id)
  .handler(async ({ data }) => (await deletePasskey(data)).status);

const failed = 'Tindakan gagal. Coba lagi.';

const root = getRouteApi('__root__');

export const Route = createFileRoute('/admin/akun')({
  loader: () => fetchOwnUser(),
  head: () => ({ meta: [{ title: 'Akun saya · Pranata' }] }),
  component: OwnUserPage,
});

function OwnUserPage() {
  const village = root.useLoaderData();
  const { sessions, passkeys } = Route.useLoaderData() ?? { sessions: [], passkeys: [] };
  const { me } = Route.useRouteContext();
  const router = useRouter();
  const sudo = useSudo();
  const submitChange = useServerFn(changePasswordFn);
  const submitSignOutOthers = useServerFn(signOutOthersFn);
  const submitStartRegistration = useServerFn(startRegistrationFn);
  const submitCreatePasskey = useServerFn(createPasskeyFn);
  const submitDeletePasskey = useServerFn(deletePasskeyFn);
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
      if (result.status === 204) {
        form.reset();
        await router.invalidate();
      }
      setMessage(
        result.status === 204
          ? 'Kata sandi telah diganti. Anda telah keluar dari semua perangkat lain.'
          : result.problem
            ? passwordProblems[result.problem]
            : failed,
      );
      return result.status;
    });
  }

  function onAddPasskey(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const form = event.currentTarget;
    const name = String(new FormData(form).get('name'));
    return sudo.run(async () => {
      const started = await submitStartRegistration();
      if (started.status !== 200) {
        if (started.status !== 428) setMessage(failed);
        return started.status;
      }
      const { ceremonyId } = started;
      let credential: Record<string, unknown>;
      try {
        credential = await createPasskeyCredential(started.options);
      } catch {
        setMessage('Passkey tidak dibuat. Peramban atau perangkat ini mungkin belum mendukungnya.');
        return 0;
      }
      const status = await submitCreatePasskey({ data: { ceremonyId, name, credential } });
      if (status === 428) return status;
      if (status === 201) {
        form.reset();
        await router.invalidate();
      }
      setMessage(
        status === 201
          ? `Passkey ${name.trim()} ditambahkan.`
          : status === 409
            ? 'Passkey ini sudah terdaftar.'
            : failed,
      );
      return status;
    });
  }

  function onDeletePasskey(id: string, name: string) {
    if (!window.confirm(`Hapus passkey ${name}?`)) return;
    return sudo.run(async () => {
      const status = await submitDeletePasskey({ data: id });
      if (status === 428) return status;
      setMessage(status === 204 ? `Passkey ${name} dihapus.` : failed);
      await router.invalidate();
      return status;
    });
  }

  async function onSignOutOthers() {
    const status = await submitSignOutOthers();
    setMessage(status === 204 ? 'Anda telah keluar dari semua perangkat lain.' : failed);
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
      <section aria-labelledby="passkeys-title">
        <h2 id="passkeys-title">Passkey</h2>
        <p>
          Masuk tanpa kata sandi dengan sidik jari, wajah, atau PIN perangkat. Kata sandi Anda tetap
          berlaku.
        </p>
        {passkeys.length > 0 ? (
          <table>
            <thead>
              <tr>
                <th>Nama</th>
                <th>Ditambahkan</th>
                <th>Terakhir dipakai</th>
                <th>Tindakan</th>
              </tr>
            </thead>
            <tbody>
              {passkeys.map((passkey) => (
                <tr key={passkey.id}>
                  <td>{passkey.name}</td>
                  <td>{time.format(new Date(passkey.createdAt))}</td>
                  <td>{passkey.lastUsedAt ? time.format(new Date(passkey.lastUsedAt)) : '—'}</td>
                  <td>
                    <button
                      type="button"
                      onClick={() => onDeletePasskey(passkey.id, passkey.name)}
                      aria-label={`Hapus passkey ${passkey.name}`}
                    >
                      Hapus
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : (
          <p>Belum ada passkey.</p>
        )}
        <form onSubmit={onAddPasskey}>
          <label>
            Nama passkey
            <input name="name" maxLength={64} placeholder="mis. Laptop kantor" required />
          </label>
          <button type="submit">Tambah passkey</button>
        </form>
      </section>
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
