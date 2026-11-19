import { createServerFn, useServerFn } from '@tanstack/react-start';
import { type FormEvent, useState } from 'react';
import { reauthenticate } from './api/generated';

const reauthenticateFn = createServerFn({ method: 'POST' })
  .validator((password: string) => password)
  .handler(async ({ data }) => (await reauthenticate({ password: data })).status);

/** An action that resolves to the API's status code. */
type Action = () => Promise<number>;

/**
 * Sudo mode: runs an action and, when the API answers 428 because the last authentication is
 * over 15 minutes old, asks for the password and then runs it again.
 */
export function useSudo() {
  const confirmPassword = useServerFn(reauthenticateFn);
  const [retry, setRetry] = useState<Action | null>(null);
  const [error, setError] = useState('');

  async function run(action: Action) {
    if ((await action()) === 428) setRetry(() => action);
  }

  async function onConfirm(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const password = String(new FormData(event.currentTarget).get('password'));
    if ((await confirmPassword({ data: password })) !== 204) {
      setError('Kata sandi salah.');
      return;
    }
    const action = retry;
    setRetry(null);
    setError('');
    if (action) await run(action);
  }

  const prompt = retry && (
    <form onSubmit={onConfirm} aria-labelledby="sudo-title">
      <h2 id="sudo-title">Konfirmasi kata sandi</h2>
      <p>Demi keamanan, masukkan lagi kata sandi Anda untuk melanjutkan.</p>
      <label>
        Kata sandi Anda
        <input
          name="password"
          type="password"
          autoComplete="current-password"
          required
          // biome-ignore lint/a11y/noAutofocus: the prompt opens in answer to the User's own action.
          autoFocus
        />
      </label>
      {error && <p role="alert">{error}</p>}
      <button type="submit">Konfirmasi</button>{' '}
      <button type="button" onClick={() => setRetry(null)}>
        Batal
      </button>
    </form>
  );

  return { run, prompt };
}
