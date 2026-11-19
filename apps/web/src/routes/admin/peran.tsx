import { createFileRoute, useRouter } from '@tanstack/react-router';
import { createServerFn, useServerFn } from '@tanstack/react-start';
import { Lock } from 'lucide-react';
import { type FormEvent, useState } from 'react';
import {
  createRole,
  listRoles,
  type Permission,
  type Role,
  type RoleInput,
  updateRole,
} from '../../api/generated';
import { Notice, PageHead } from '../../components/admin';
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
import { useSudo } from '../../sudo';

// Server-only; the Roles, or the status that refused them.
const fetchRoles = createServerFn({ method: 'GET' }).handler(async () => {
  const response = await listRoles();
  return response.status === 200 ? response.data : response.status;
});

const saveRoleFn = createServerFn({ method: 'POST' })
  .validator((data: { id: string | undefined; role: RoleInput }) => data)
  .handler(async ({ data }) =>
    data.id ? (await updateRole(data.id, data.role)).status : (await createRole(data.role)).status,
  );

export const Route = createFileRoute('/admin/peran')({
  loader: () => fetchRoles(),
  head: () => ({ meta: [{ title: 'Peran · Pranata' }] }),
  component: Roles,
});

// Every Permission in the catalog, as the Record type demands; the generated constant would pull
// the server-only client into the browser bundle.
const permissionLabels: Record<Permission, string> = {
  'village.manage': 'Kelola data desa',
  'content.write': 'Tulis draf konten',
  'content.publish': 'Terbitkan konten',
  'complaint.read': 'Baca pengaduan',
  'complaint.handle': 'Tangani pengaduan',
  'audit.read': 'Baca log audit',
};

const saveErrors: Record<number, string> = {
  409: 'Nama peran itu sudah dipakai.',
  422: 'Nama peran wajib diisi.',
};

function Roles() {
  const roles = Route.useLoaderData();
  const router = useRouter();
  const sudo = useSudo();
  const save = useServerFn(saveRoleFn);
  const [message, setMessage] = useState('');

  if (typeof roles === 'number') {
    return (
      <>
        <PageHead title="Peran" />
        <Notice error>Hanya Administrator yang dapat mengelola peran.</Notice>
      </>
    );
  }

  function onSave(event: FormEvent<HTMLFormElement>, id?: string) {
    event.preventDefault();
    const formElement = event.currentTarget;
    const form = new FormData(formElement);
    const role = {
      name: String(form.get('name')),
      permissions: form.getAll('permissions').map(String) as Permission[],
    };
    return sudo.run(async () => {
      const status = await save({ data: { id, role } });
      if (status === 428) return status;
      setMessage(saveErrors[status] ?? (status < 300 ? 'Peran tersimpan.' : 'Gagal menyimpan.'));
      if (status < 300) {
        if (!id) formElement.reset();
        await router.invalidate();
      }
      return status;
    });
  }

  return (
    <>
      <PageHead title="Peran" />
      {sudo.prompt}
      {message && <Notice>{message}</Notice>}
      {roles.map((role) =>
        role.administrator ? (
          <Card key={role.id} role="region" aria-labelledby={`role-${role.id}`}>
            <CardHeader>
              <h2 id={`role-${role.id}`} className="text-base font-semibold">
                {role.name}
              </h2>
              <CardDescription>
                Bawaan dan terkunci: memegang semua izin serta mengelola pengguna dan peran.
              </CardDescription>
              <CardAction>
                <Lock className="size-4 text-muted-foreground" aria-hidden="true" />
              </CardAction>
            </CardHeader>
          </Card>
        ) : (
          // Keyed by content so a saved change resets the form to the stored Role.
          <RoleForm
            key={`${role.id}:${role.name}:${role.permissions.join()}`}
            role={role}
            onSubmit={(event) => onSave(event, role.id)}
          />
        ),
      )}
      <RoleForm onSubmit={(event) => onSave(event)} />
    </>
  );
}

function RoleForm({
  role,
  onSubmit,
}: {
  role?: Role;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
}) {
  const title = role ? role.name : 'Peran baru';
  const id = role?.id ?? 'new';
  return (
    <Card>
      <form onSubmit={onSubmit} aria-label={title} className="contents">
        <CardHeader>
          <h2 className="text-base font-semibold">{title}</h2>
        </CardHeader>
        <CardContent className="grid gap-5">
          <div className="grid max-w-xs gap-2">
            <Label htmlFor={`role-name-${id}`}>Nama peran</Label>
            <Input id={`role-name-${id}`} name="name" defaultValue={role?.name} required />
          </div>
          <fieldset className="grid gap-2.5 sm:grid-cols-2 xl:grid-cols-3">
            <legend className="mb-2.5 text-[0.8125rem] font-semibold text-muted-foreground">
              Izin
            </legend>
            {(Object.keys(permissionLabels) as Permission[]).map((permission) => (
              <label key={permission} className="flex items-start gap-2.5">
                <input
                  type="checkbox"
                  name="permissions"
                  value={permission}
                  defaultChecked={role?.permissions.includes(permission)}
                  className="mt-0.5 size-4 accent-primary"
                />
                {permissionLabels[permission]}
              </label>
            ))}
          </fieldset>
          <Button type="submit" size="lg" className="w-fit px-4">
            {role ? 'Simpan' : 'Buat peran'}
          </Button>
        </CardContent>
      </form>
    </Card>
  );
}
