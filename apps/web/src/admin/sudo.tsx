import {
	createContext,
	type FormEvent,
	type ReactNode,
	useCallback,
	useContext,
	useMemo,
	useRef,
	useState,
} from "react";
import { withSudo } from "@/admin/sudo-retry";
import { useConfirmSudo } from "@/api/generated/auth/auth";
import { Button } from "@/components/ui/button";
import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogFooter,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import { Field, FieldError, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { messageFor } from "@/lib/errors";

type RunWithSudo = <T>(run: () => Promise<T>) => Promise<T>;

const SudoContext = createContext<RunWithSudo | null>(null);

// Returns a wrapper that runs a request and, when the API answers
// `sudo_required`, asks for the password and retries once.
export function useSudo(): RunWithSudo {
	const value = useContext(SudoContext);
	if (!value) throw new Error("useSudo must be used inside SudoProvider");
	return value;
}

export function SudoProvider({ children }: Readonly<{ children: ReactNode }>) {
	const [open, setOpen] = useState(false);
	const [password, setPassword] = useState("");
	const [error, setError] = useState<string | null>(null);
	const settle = useRef<((confirmed: boolean) => void) | null>(null);
	const confirmSudo = useConfirmSudo();

	const finish = useCallback((confirmed: boolean) => {
		settle.current?.(confirmed);
		settle.current = null;
		setOpen(false);
		setPassword("");
		setError(null);
	}, []);

	const ask = useCallback(
		() =>
			new Promise<boolean>((resolve) => {
				settle.current?.(false);
				settle.current = resolve;
				setOpen(true);
			}),
		[],
	);

	const run = useCallback<RunWithSudo>((fn) => withSudo(fn, ask), [ask]);
	const value = useMemo(() => run, [run]);

	function submit(event: FormEvent) {
		event.preventDefault();
		setError(null);
		confirmSudo.mutate(
			{ data: { password } },
			{
				onSuccess: () => finish(true),
				onError: (e) => setError(messageFor(e.messageCode)),
			},
		);
	}

	return (
		<SudoContext.Provider value={value}>
			{children}
			<Dialog
				open={open}
				onOpenChange={(next) => {
					if (!next) finish(false);
				}}
			>
				<DialogContent>
					<form onSubmit={submit} className="grid gap-4">
						<DialogHeader>
							<DialogTitle>Konfirmasi kata sandi</DialogTitle>
							<DialogDescription>
								Untuk keamanan, masukkan kata sandi Anda sekali lagi.
							</DialogDescription>
						</DialogHeader>
						<Field data-invalid={error ? true : undefined}>
							<FieldLabel htmlFor="sudo-password">Kata sandi</FieldLabel>
							<Input
								id="sudo-password"
								type="password"
								autoComplete="current-password"
								autoFocus
								required
								value={password}
								aria-invalid={error ? true : undefined}
								onChange={(e) => setPassword(e.target.value)}
							/>
							{error ? <FieldError role="alert">{error}</FieldError> : null}
						</Field>
						<DialogFooter>
							<Button
								type="button"
								variant="outline"
								onClick={() => finish(false)}
							>
								Batal
							</Button>
							<Button
								type="submit"
								disabled={confirmSudo.isPending || !password}
							>
								Konfirmasi
							</Button>
						</DialogFooter>
					</form>
				</DialogContent>
			</Dialog>
		</SudoContext.Provider>
	);
}
