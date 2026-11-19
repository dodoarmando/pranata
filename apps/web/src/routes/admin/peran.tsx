import { createFileRoute, useRouter } from '@tanstack/react-router';
import { createServerFn, useServerFn } from '@tanstack/react-start';
import { type FormEvent, useState } from 'react';
import {
  createRole,
  listRoles,
  type Permission,
  type Role,
  type RoleInput,
  updateRole,
} from '../../api/generated';
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
        <h1>Peran</h1>
        <p role="alert">Hanya Administrator yang dapat mengelola peran.</p>
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
      <h1>Peran</h1>
      {sudo.prompt}
      {message && <p role="alert">{message}</p>}
      {roles.map((role) =>
        role.administrator ? (
          <section key={role.id} aria-labelledby={`role-${role.id}`}>
            <h2 id={`role-${role.id}`}>{role.name}</h2>
            <p>Bawaan dan terkunci: memegang semua izin serta mengelola pengguna dan peran.</p>
          </section>
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
  return (
    <form onSubmit={onSubmit} aria-label={title}>
      <h2>{title}</h2>
      <label>
        Nama peran
        <input name="name" defaultValue={role?.name} required />
      </label>
      <fieldset>
        <legend>Izin</legend>
        {(Object.keys(permissionLabels) as Permission[]).map((permission) => (
          <label key={permission}>
            <input
              type="checkbox"
              name="permissions"
              value={permission}
              defaultChecked={role?.permissions.includes(permission)}
            />{' '}
            {permissionLabels[permission]}
          </label>
        ))}
      </fieldset>
      <button type="submit">{role ? 'Simpan' : 'Buat peran'}</button>
    </form>
  );
}
