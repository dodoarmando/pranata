import { createFileRoute, getRouteApi, useHydrated, useNavigate } from '@tanstack/react-router';
import { createServerFn, useServerFn } from '@tanstack/react-start';
import { type FormEvent, type MouseEvent, useState } from 'react';
import {
  type PasskeySignIn,
  type SignIn,
  signIn,
  signInWithPasskey,
  startPasskeySignIn,
} from '../api/generated';
import { getPasskeyCredential } from '../passkey';
import { villageTitle } from '../village';

const signInFn = createServerFn({ method: 'POST' })
  .validator((data: SignIn) => data)
  .handler(async ({ data }) => (await signIn(data)).status);

// Every username gets a challenge, a decoy when it has no passkeys; null only when the API is busy.
const startPasskeySignInFn = createServerFn({ method: 'POST' })
  .validator((username: string) => username)
  .handler(async ({ data }) => {
    const response = await startPasskeySignIn({ username: data });
    if (response.status !== 200) return null;
    const { ceremonyId, options } = response.data;
    return { ceremonyId, options: JSON.stringify(options) };
  });

const passkeySignInFn = createServerFn({ method: 'POST' })
  .validator((data: PasskeySignIn) => data)
  .handler(async ({ data }) => (await signInWithPasskey(data)).status);

const root = getRouteApi('__root__');

export const Route = createFileRoute('/masuk')({
  head: () => ({ meta: [{ title: 'Masuk · Pranata' }] }),
  component: SignInPage,
});

function SignInPage() {
  const village = root.useLoaderData();
  const navigate = useNavigate();
  const submit = useServerFn(signInFn);
  const startPasskey = useServerFn(startPasskeySignInFn);
  const submitPasskey = useServerFn(passkeySignInFn);
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

  // The same message whatever failed, so it never tells whether the username exists.
  async function onPasskey(event: MouseEvent<HTMLButtonElement>) {
    const form = new FormData(event.currentTarget.form ?? undefined);
    const username = String(form.get('username')).trim();
    if (!username) {
      setError('Masukkan nama pengguna terlebih dahulu.');
      return;
    }
    setError('');
    setBusy(true);
    const challenge = await startPasskey({ data: username });
    let status = 0;
    if (challenge) {
      try {
        const credential = await getPasskeyCredential(challenge.options);
        const remember = form.get('remember') === 'on';
        const { ceremonyId } = challenge;
        status = await submitPasskey({ data: { ceremonyId, credential, remember } });
      } catch {
        // Cancelled, or no passkey on this device.
      }
    }
    setBusy(false);
    if (status === 204) {
      await navigate({ to: '/admin' });
    } else {
      setError('Tidak dapat masuk dengan passkey. Coba lagi, atau masuk dengan kata sandi.');
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
        <button type="button" onClick={onPasskey} disabled={busy || !hydrated}>
          Masuk dengan passkey
        </button>
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
