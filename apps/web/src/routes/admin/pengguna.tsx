import { createFileRoute, useRouter } from '@tanstack/react-router';
import { createServerFn, useServerFn } from '@tanstack/react-start';
import { Plus } from 'lucide-react';
import { type FormEvent, useState } from 'react';
import {
  createAccessLink,
  createUser,
  linkUserOfficial,
  listPositions,
  listRoles,
  listUsers,
  type NewUser,
  removePasskeys,
  revokeSessions,
  type UserChange,
  updateUser,
} from '../../api/generated';
import { DataTable, Notice, PageHead } from '../../components/admin';
import { Badge } from '../../components/ui/badge';
import { Button } from '../../components/ui/button';
import { Card, CardContent, CardHeader } from '../../components/ui/card';
import { Input } from '../../components/ui/input';
import { Label } from '../../components/ui/label';
import { NativeSelect, NativeSelectOption } from '../../components/ui/native-select';
import { TableBody, TableCell, TableHead, TableHeader, TableRow } from '../../components/ui/table';
import { useSudo } from '../../sudo';

// Server-only; the Users, Roles and Officials, or the status that refused them.
const fetchUsers = createServerFn({ method: 'GET' }).handler(async () => {
  const [users, roles, positions] = await Promise.all([listUsers(), listRoles(), listPositions()]);
  if (users.status !== 200) return users.status;
  if (roles.status !== 200) return roles.status;
  // Each Official once, named as on any of their Tenures.
  const officials = new Map(
    (positions.status === 200 ? positions.data : [])
      .flatMap((position) => position.tenures)
      .map((tenure) => [tenure.officialId, tenure.name]),
  );
  return {
    users: users.data,
    roles: roles.data,
    officials: [...officials]
      .map(([id, name]) => ({ id, name }))
      .sort((a, b) => a.name.localeCompare(b.name, 'id')),
  };
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

const linkOfficialFn = createServerFn({ method: 'POST' })
  .validator((data: { id: string; officialId: string | null }) => data)
  .handler(
    async ({ data }) => (await linkUserOfficial(data.id, { officialId: data.officialId })).status,
  );

const revokeSessionsFn = createServerFn({ method: 'POST' })
  .validator((id: string) => id)
  .handler(async ({ data }) => (await revokeSessions(data)).status);

const removePasskeysFn = createServerFn({ method: 'POST' })
  .validator((id: string) => id)
  .handler(async ({ data }) => (await removePasskeys(data)).status);

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

const linkErrors: Record<number, string> = {
  409: 'Pejabat itu sudah ditautkan ke pengguna lain.',
};

function Users() {
  const page = Route.useLoaderData();
  const router = useRouter();
  const sudo = useSudo();
  const submitCreate = useServerFn(createUserFn);
  const submitReset = useServerFn(resetFn);
  const submitUpdate = useServerFn(updateUserFn);
  const submitLink = useServerFn(linkOfficialFn);
  const submitRevoke = useServerFn(revokeSessionsFn);
  const submitRemovePasskeys = useServerFn(removePasskeysFn);
  const [link, setLink] = useState<{ username: string; url: string } | null>(null);
  const [message, setMessage] = useState('');

  if (typeof page === 'number') {
    return (
      <>
        <PageHead title="Pengguna" />
        <Notice error>Hanya Administrator yang dapat mengelola pengguna.</Notice>
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
      <PageHead title="Pengguna" />
      {sudo.prompt}
      {message && <Notice>{message}</Notice>}
      {link && (
        <Card role="region" aria-labelledby="link-title">
          <CardHeader>
            <h2 id="link-title" className="text-base font-semibold">
              Tautan Akses untuk {link.username}
            </h2>
          </CardHeader>
          <CardContent className="grid gap-2">
            <Input readOnly value={link.url} aria-label="Tautan Akses" className="font-mono" />
            <p className="text-muted-foreground">
              Berikan tautan ini langsung kepada pengguna. Tautan berlaku 24 jam, hanya sekali
              pakai, dan tidak ditampilkan lagi.
            </p>
          </CardContent>
        </Card>
      )}
      <Card>
        <form onSubmit={onCreate} aria-labelledby="new-user-title" className="contents">
          <CardHeader>
            <h2 id="new-user-title" className="text-base font-semibold">
              Pengguna baru
            </h2>
          </CardHeader>
          <CardContent className="flex flex-wrap items-end gap-4">
            <div className="grid w-full max-w-xs gap-2">
              <Label htmlFor="new-username">Nama pengguna</Label>
              <Input id="new-username" name="username" autoCapitalize="none" required />
            </div>
            <div className="grid gap-2">
              <Label htmlFor="new-role">Peran</Label>
              <NativeSelect id="new-role" name="roleId" className="w-48" required>
                {page.roles.map((role) => (
                  <NativeSelectOption key={role.id} value={role.id}>
                    {role.name}
                  </NativeSelectOption>
                ))}
              </NativeSelect>
            </div>
            <Button type="submit" size="lg" className="px-4">
              <Plus aria-hidden="true" />
              Buat pengguna
            </Button>
          </CardContent>
        </form>
      </Card>
      <DataTable>
        <TableHeader>
          <TableRow>
            <TableHead>Nama pengguna</TableHead>
            <TableHead>Peran</TableHead>
            <TableHead>Pejabat</TableHead>
            <TableHead>Status</TableHead>
            <TableHead>Tindakan</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {page.users.map((user) => (
            <TableRow key={user.id}>
              <TableCell className="font-medium">{user.username}</TableCell>
              <TableCell>
                <NativeSelect
                  aria-label={`Peran ${user.username}`}
                  value={user.roleId}
                  onChange={(event) => {
                    const change = { roleId: event.target.value };
                    act(updateErrors, () => submitUpdate({ data: { id: user.id, change } }))();
                  }}
                >
                  {page.roles.map((role) => (
                    <NativeSelectOption key={role.id} value={role.id}>
                      {role.name}
                    </NativeSelectOption>
                  ))}
                </NativeSelect>
              </TableCell>
              <TableCell>
                <NativeSelect
                  aria-label={`Pejabat ${user.username}`}
                  value={user.officialId ?? ''}
                  onChange={(event) => {
                    const officialId = event.target.value || null;
                    act(linkErrors, () => submitLink({ data: { id: user.id, officialId } }))();
                  }}
                >
                  <NativeSelectOption value="">Bukan pejabat</NativeSelectOption>
                  {page.officials.map((official) => (
                    <NativeSelectOption key={official.id} value={official.id}>
                      {official.name}
                    </NativeSelectOption>
                  ))}
                </NativeSelect>
              </TableCell>
              <TableCell>
                <Badge variant={user.active ? 'secondary' : 'outline'}>
                  {user.active ? 'Aktif' : 'Nonaktif'}
                </Badge>
              </TableCell>
              <TableCell>
                <div className="flex gap-2">
                  {user.active && (
                    <Button
                      type="button"
                      variant="outline"
                      onClick={act({}, async () => {
                        const result = await submitReset({ data: user.id });
                        if (result.token) showLink(user.username, result.token);
                        return result.status;
                      })}
                    >
                      Buat Tautan Akses
                    </Button>
                  )}
                  <Button
                    type="button"
                    variant="outline"
                    onClick={act(
                      {},
                      () => submitRevoke({ data: user.id }),
                      `Semua sesi ${user.username} telah dicabut.`,
                    )}
                  >
                    Cabut semua sesi
                  </Button>
                  <Button
                    type="button"
                    variant="outline"
                    disabled={user.passkeys === 0}
                    onClick={() =>
                      window.confirm(
                        `Hapus semua passkey ${user.username}? Kata sandinya tetap berlaku.`,
                      ) &&
                      act(
                        {},
                        () => submitRemovePasskeys({ data: user.id }),
                        `Semua passkey ${user.username} telah dihapus.`,
                      )()
                    }
                  >
                    Hapus passkey ({user.passkeys})
                  </Button>
                  {user.active ? (
                    <Button
                      type="button"
                      variant="destructive"
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
                    </Button>
                  ) : (
                    <Button
                      type="button"
                      variant="outline"
                      onClick={act(updateErrors, () =>
                        submitUpdate({ data: { id: user.id, change: { active: true } } }),
                      )}
                    >
                      Aktifkan
                    </Button>
                  )}
                </div>
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </DataTable>
    </>
  );
}
