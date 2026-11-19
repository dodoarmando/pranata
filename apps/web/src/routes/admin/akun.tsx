import { createFileRoute, getRouteApi, useRouter } from '@tanstack/react-router';
import { createServerFn, useServerFn } from '@tanstack/react-start';
import { Plus, Trash2 } from 'lucide-react';
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
import { DataTable, Notice, PageHead } from '../../components/admin';
import { Badge } from '../../components/ui/badge';
import { Button } from '../../components/ui/button';
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardHeader,
} from '../../components/ui/card';
import { Input } from '../../components/ui/input';
import { Label } from '../../components/ui/label';
import { TableBody, TableCell, TableHead, TableHeader, TableRow } from '../../components/ui/table';
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
      <PageHead title="Akun saya">
        <strong className="font-medium text-foreground">{me.username}</strong> {me.role}
      </PageHead>
      {sudo.prompt}
      {message && <Notice>{message}</Notice>}
      <Card>
        <form onSubmit={onChangePassword} aria-labelledby="password-title" className="contents">
          <CardHeader>
            <h2 id="password-title" className="text-base font-semibold">
              Ganti kata sandi
            </h2>
          </CardHeader>
          <CardContent className="grid max-w-md gap-2">
            <Label htmlFor="new-password">Kata sandi baru</Label>
            <Input
              id="new-password"
              name="password"
              type="password"
              autoComplete="new-password"
              minLength={12}
              aria-describedby="password-hint"
              required
            />
            <p id="password-hint" className="text-muted-foreground">
              {passwordHint}
            </p>
            <Button type="submit" size="lg" className="mt-2 w-fit px-4">
              Ganti kata sandi
            </Button>
          </CardContent>
        </form>
      </Card>
      <Card role="region" aria-labelledby="passkeys-title">
        <CardHeader>
          <h2 id="passkeys-title" className="text-base font-semibold">
            Passkey
          </h2>
          <CardDescription>
            Masuk tanpa kata sandi dengan sidik jari, wajah, atau PIN perangkat. Kata sandi Anda
            tetap berlaku.
          </CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4">
          {passkeys.length > 0 ? (
            <DataTable>
              <TableHeader>
                <TableRow>
                  <TableHead>Nama</TableHead>
                  <TableHead>Ditambahkan</TableHead>
                  <TableHead>Terakhir dipakai</TableHead>
                  <TableHead className="text-right">Tindakan</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {passkeys.map((passkey) => (
                  <TableRow key={passkey.id}>
                    <TableCell className="font-medium">{passkey.name}</TableCell>
                    <TableCell>{time.format(new Date(passkey.createdAt))}</TableCell>
                    <TableCell>
                      {passkey.lastUsedAt ? time.format(new Date(passkey.lastUsedAt)) : '—'}
                    </TableCell>
                    <TableCell className="text-right">
                      <Button
                        type="button"
                        variant="outline"
                        onClick={() => onDeletePasskey(passkey.id, passkey.name)}
                        aria-label={`Hapus passkey ${passkey.name}`}
                      >
                        <Trash2 aria-hidden="true" />
                        Hapus
                      </Button>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </DataTable>
          ) : (
            <p className="text-muted-foreground">Belum ada passkey.</p>
          )}
          <form onSubmit={onAddPasskey} className="flex flex-wrap items-end gap-2">
            <div className="grid w-full max-w-xs gap-2">
              <Label htmlFor="passkey-name">Nama passkey</Label>
              <Input
                id="passkey-name"
                name="name"
                maxLength={64}
                placeholder="mis. Laptop kantor"
                required
              />
            </div>
            <Button type="submit" variant="outline" size="lg" className="px-4">
              <Plus aria-hidden="true" />
              Tambah passkey
            </Button>
          </form>
        </CardContent>
      </Card>
      <Card role="region" aria-labelledby="sessions-title">
        <CardHeader>
          <h2 id="sessions-title" className="text-base font-semibold">
            Sesi aktif
          </h2>
          <CardAction>
            <Button
              type="button"
              variant="outline"
              onClick={onSignOutOthers}
              disabled={sessions.length < 2}
            >
              Keluar dari perangkat lain
            </Button>
          </CardAction>
        </CardHeader>
        <CardContent>
          <DataTable>
            <TableHeader>
              <TableRow>
                <TableHead>Perangkat</TableHead>
                <TableHead>Alamat IP</TableHead>
                <TableHead>Aktivitas terakhir</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {sessions.map((session) => (
                <TableRow key={`${session.lastSeenAt}-${session.userAgent}-${session.ip}`}>
                  <TableCell>
                    {describeDevice(session.userAgent)}
                    {session.current && (
                      <Badge variant="secondary" className="ml-2">
                        perangkat ini
                      </Badge>
                    )}
                  </TableCell>
                  <TableCell>{session.ip ?? '—'}</TableCell>
                  <TableCell>{time.format(new Date(session.lastSeenAt))}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </DataTable>
        </CardContent>
      </Card>
    </>
  );
}
