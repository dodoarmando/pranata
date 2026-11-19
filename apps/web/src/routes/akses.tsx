import { createFileRoute, Link, useHydrated } from '@tanstack/react-router';
import { createServerFn, useServerFn } from '@tanstack/react-start';
import { type FormEvent, useState } from 'react';
import { useAccessLink as setPasswordFromAccessLink, type UseAccessLink } from '../api/generated';
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
      <main>
        <h1>Kata sandi tersimpan</h1>
        <p>
          Silakan <Link to="/masuk">masuk</Link> dengan nama pengguna dan kata sandi baru Anda.
        </p>
      </main>
    );
  }
  return (
    <main>
      <h1>Buat kata sandi</h1>
      <form method="post" onSubmit={onSubmit}>
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
        {error && <p role="alert">{error}</p>}
        <button type="submit" disabled={busy || !hydrated}>
          Simpan kata sandi
        </button>
      </form>
    </main>
  );
}
