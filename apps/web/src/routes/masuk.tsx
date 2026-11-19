import { createFileRoute, getRouteApi, useHydrated, useNavigate } from '@tanstack/react-router';
import { createServerFn, useServerFn } from '@tanstack/react-start';
import { type FormEvent, useState } from 'react';
import { type SignIn, signIn } from '../api/generated';
import { villageTitle } from '../village';

const signInFn = createServerFn({ method: 'POST' })
  .validator((data: SignIn) => data)
  .handler(async ({ data }) => (await signIn(data)).status);

const root = getRouteApi('__root__');

export const Route = createFileRoute('/masuk')({
  head: () => ({ meta: [{ title: 'Masuk · Pranata' }] }),
  component: SignInPage,
});

function SignInPage() {
  const village = root.useLoaderData();
  const navigate = useNavigate();
  const submit = useServerFn(signInFn);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  // Before hydration a click would post the form natively, without the handler.
  const hydrated = useHydrated();

  async function onSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const form = new FormData(event.currentTarget);
    setBusy(true);
    const status = await submit({
      data: {
        username: String(form.get('username')),
        password: String(form.get('password')),
        remember: form.get('remember') === 'on',
      },
    });
    setBusy(false);
    if (status === 204) {
      await navigate({ to: '/admin' });
    } else {
      setError('Nama pengguna atau kata sandi salah.');
    }
  }

  return (
    <main>
      <h1>Masuk Pengelola Situs</h1>
      {village && <p>Pemerintah {villageTitle(village)}</p>}
      <form method="post" onSubmit={onSubmit}>
        <label>
          Nama pengguna
          <input
            name="username"
            autoComplete="username"
            autoCapitalize="none"
            placeholder="Masukkan nama pengguna Anda"
            required
          />
        </label>
        <label>
          Kata sandi
          <input
            name="password"
            type="password"
            autoComplete="current-password"
            placeholder="Masukkan kata sandi Anda"
            required
          />
        </label>
        <details>
          <summary>Lupa kata sandi?</summary>
          <p>Hubungi Administrator desa Anda untuk meminta Tautan Akses baru.</p>
        </details>
        <label>
          <input name="remember" type="checkbox" /> Ingat saya di perangkat ini
        </label>
        {error && <p role="alert">{error}</p>}
        <button type="submit" disabled={busy || !hydrated}>
          Masuk
        </button>
      </form>
      <a href="/">Kembali ke situs desa</a>
    </main>
  );
}
