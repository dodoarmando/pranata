import { createFileRoute, useHydrated, useNavigate } from '@tanstack/react-router';
import { createServerFn, useServerFn } from '@tanstack/react-start';
import { KeyRound, LogIn } from 'lucide-react';
import { type FormEvent, type MouseEvent, useState } from 'react';
import {
  type PasskeySignIn,
  type SignIn,
  signIn,
  signInWithPasskey,
  startPasskeySignIn,
} from '../api/generated';
import { Notice } from '../components/admin';
import { AuthLayout, authInput, authSubmit } from '../components/auth-layout';
import { Button } from '../components/ui/button';
import { Input } from '../components/ui/input';
import { Label } from '../components/ui/label';
import { getPasskeyCredential } from '../passkey';

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

export const Route = createFileRoute('/masuk')({
  head: () => ({ meta: [{ title: 'Masuk · Pranata' }] }),
  component: SignInPage,
});

function SignInPage() {
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
    <AuthLayout title="Masuk Pengelola Situs">
      <form method="post" onSubmit={onSubmit} className="flex flex-col gap-5">
        <div className="grid gap-2">
          <Label htmlFor="username">Nama pengguna</Label>
          <Input
            id="username"
            name="username"
            autoComplete="username"
            autoCapitalize="none"
            placeholder="Masukkan nama pengguna Anda"
            className={authInput}
            required
          />
        </div>
        <div className="grid gap-2">
          <Label htmlFor="password">Kata sandi</Label>
          <Input
            id="password"
            name="password"
            type="password"
            autoComplete="current-password"
            placeholder="Masukkan kata sandi Anda"
            className={authInput}
            required
          />
          <details className="text-sm">
            <summary className="w-fit cursor-pointer font-medium text-primary hover:underline">
              Lupa kata sandi?
            </summary>
            <p className="mt-2 text-muted-foreground">
              Hubungi Administrator desa Anda untuk meminta Tautan Akses baru.
            </p>
          </details>
        </div>
        <label className="flex items-center gap-2.5 text-sm">
          <input name="remember" type="checkbox" className="size-4.5 accent-primary" /> Ingat saya
          di perangkat ini
        </label>
        {error && <Notice error>{error}</Notice>}
        <Button type="submit" className={authSubmit} disabled={busy || !hydrated}>
          Masuk
          <LogIn aria-hidden="true" />
        </Button>
        <div className="flex flex-col items-center gap-4">
          <div className="flex w-full items-center gap-3 text-sm text-muted-foreground before:h-px before:flex-1 before:bg-border after:h-px after:flex-1 after:bg-border">
            atau
          </div>
          <Button
            type="button"
            variant="outline"
            className="h-11 px-4 text-[0.9375rem]"
            onClick={onPasskey}
            disabled={busy || !hydrated}
          >
            Masuk dengan passkey
            <KeyRound aria-hidden="true" />
          </Button>
        </div>
      </form>
    </AuthLayout>
  );
}
