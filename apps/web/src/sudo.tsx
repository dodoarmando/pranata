import { createServerFn, useServerFn } from '@tanstack/react-start';
import { type FormEvent, useState } from 'react';
import { reauthenticate } from './api/generated';
import { Notice } from './components/admin';
import { Button } from './components/ui/button';
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from './components/ui/dialog';
import { Input } from './components/ui/input';
import { Label } from './components/ui/label';

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

  const prompt = (
    <Dialog
      open={retry !== null}
      onOpenChange={(open) => {
        if (open) return;
        setRetry(null);
        setError('');
      }}
    >
      <DialogContent showCloseButton={false}>
        <form onSubmit={onConfirm} className="grid gap-4">
          <DialogHeader>
            <DialogTitle>Konfirmasi kata sandi</DialogTitle>
            <DialogDescription>
              Demi keamanan, masukkan lagi kata sandi Anda untuk melanjutkan.
            </DialogDescription>
          </DialogHeader>
          <div className="grid gap-2">
            <Label htmlFor="sudo-password">Kata sandi Anda</Label>
            <Input
              id="sudo-password"
              name="password"
              type="password"
              autoComplete="current-password"
              required
            />
          </div>
          {error && <Notice error>{error}</Notice>}
          <DialogFooter>
            <DialogClose
              render={<Button type="button" variant="outline" size="lg" className="px-4" />}
            >
              Batal
            </DialogClose>
            <Button type="submit" size="lg" className="px-4">
              Konfirmasi
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );

  return { run, prompt };
}
