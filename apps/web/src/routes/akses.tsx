import { createFileRoute, Link, useHydrated } from '@tanstack/react-router';
import { createServerFn, useServerFn } from '@tanstack/react-start';
import { type FormEvent, useState } from 'react';
import { useAccessLink as setPasswordFromAccessLink, type UseAccessLink } from '../api/generated';
import { Notice } from '../components/admin';
import { AuthLayout, authInput, authSubmit } from '../components/auth-layout';
import { Button } from '../components/ui/button';
import { Input } from '../components/ui/input';
import { Label } from '../components/ui/label';
import { passwordHint, passwordProblems } from '../password';

const setPasswordFn = createServerFn({ method: 'POST' })
  .validator((data: UseAccessLink) => data)
  .handler(async ({ data }) => {
    const response = await setPasswordFromAccessLink(data);
    return response.status === 422
      ? { status: response.status, problem: response.data.problem }
      : { status: response.status };
  });

export const Route = createFileRoute('/akses')({
  head: () => ({ meta: [{ title: 'Buat kata sandi · Pranata' }] }),
  component: AccessLinkPage,
});

function AccessLinkPage() {
  const submit = useServerFn(setPasswordFn);
  const [error, setError] = useState('');
  const [done, setDone] = useState(false);
  const [busy, setBusy] = useState(false);
  // Before hydration a click would post the form natively, without the handler.
  const hydrated = useHydrated();

  async function onSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const password = String(new FormData(event.currentTarget).get('password'));
    setBusy(true);
    // The token sits in the fragment, so it never reaches a server log.
    const result = await submit({ data: { token: location.hash.slice(1), password } });
    setBusy(false);
    if (result.status === 204) {
      setDone(true);
    } else if (result.problem) {
      setError(passwordProblems[result.problem]);
    } else {
      setError('Tautan Akses ini tidak berlaku lagi. Minta tautan baru ke Administrator desa.');
    }
  }

  if (done) {
    return (
      <AuthLayout title="Kata sandi tersimpan">
        <p className="text-muted-foreground">
          Silakan{' '}
          <Link to="/masuk" className="font-medium text-primary hover:underline">
            masuk
          </Link>{' '}
          dengan nama pengguna dan kata sandi baru Anda.
        </p>
      </AuthLayout>
    );
  }
  return (
    <AuthLayout title="Buat kata sandi">
      <form method="post" onSubmit={onSubmit} className="flex flex-col gap-5">
        <div className="grid gap-2">
          <Label htmlFor="password">Kata sandi baru</Label>
          <Input
            id="password"
            name="password"
            type="password"
            autoComplete="new-password"
            minLength={12}
            aria-describedby="password-hint"
            className={authInput}
            required
          />
          <p id="password-hint" className="text-sm text-muted-foreground">
            {passwordHint}
          </p>
        </div>
        {error && <Notice error>{error}</Notice>}
        <Button type="submit" className={authSubmit} disabled={busy || !hydrated}>
          Simpan kata sandi
        </Button>
      </form>
    </AuthLayout>
  );
}
