import { createFileRoute, useRouter } from "@tanstack/react-router";
import { EyeIcon, EyeOffIcon, KeyRoundIcon } from "lucide-react";
import { type FormEvent, useEffect, useState } from "react";
import { ApiError } from "@/api/fetcher";
import { useLogin } from "@/api/generated/auth/auth";
import { AuthShell } from "@/components/auth-shell";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogHeader,
	DialogTitle,
	DialogTrigger,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Separator } from "@/components/ui/separator";
import { setCsrfToken } from "@/lib/csrf";
import { messageFor } from "@/lib/errors";
import { passkeysSupported, signInWithPasskey } from "@/lib/passkey";
import { AppQueryProvider } from "@/lib/query-client-provider";
import { safeAdminRedirect } from "@/lib/safe-redirect";

export const Route = createFileRoute("/masuk")({
	validateSearch: (search: Record<string, unknown>): { ke?: string } =>
		typeof search.ke === "string" ? { ke: search.ke } : {},
	head: () => ({ meta: [{ title: "Masuk Pengelola Situs" }] }),
	component: SignInPage,
});

function SignInPage() {
	return (
		<AppQueryProvider>
			<SignIn />
		</AppQueryProvider>
	);
}

function SignIn() {
	const { ke } = Route.useSearch();
	const router = useRouter();
	const [showPassword, setShowPassword] = useState(false);
	const [remember, setRemember] = useState(false);
	// The page is server-rendered, so support is only known after mount.
	const [supported, setSupported] = useState(false);
	const [passkeyPending, setPasskeyPending] = useState(false);
	const [passkeyError, setPasskeyError] = useState<string | null>(null);
	useEffect(() => setSupported(passkeysSupported()), []);

	function signedIn(session: { csrf_token: string }) {
		setCsrfToken(session.csrf_token);
		router.history.push(safeAdminRedirect(ke));
	}

	const login = useLogin({ mutation: { onSuccess: signedIn } });

	async function onPasskey() {
		setPasskeyError(null);
		setPasskeyPending(true);
		try {
			const session = await signInWithPasskey(remember);
			if (session) signedIn(session);
		} catch (error) {
			setPasskeyError(
				messageFor(
					error instanceof ApiError ? error.messageCode : "passkey_failed",
				),
			);
		} finally {
			setPasskeyPending(false);
		}
	}

	function onSubmit(event: FormEvent<HTMLFormElement>) {
		event.preventDefault();
		const form = new FormData(event.currentTarget);
		login.mutate({
			data: {
				username: String(form.get("username") ?? "").trim(),
				password: String(form.get("password") ?? ""),
				remember,
			},
		});
	}

	return (
		<AuthShell title="Masuk Pengelola Situs" subtitle="Pemerintah Desa">
			<form onSubmit={onSubmit} className="flex flex-col gap-4" noValidate>
				<div className="flex flex-col gap-2">
					<Label htmlFor="username">Nama pengguna</Label>
					<Input
						id="username"
						name="username"
						autoComplete="username"
						placeholder="Masukkan nama pengguna"
						required
					/>
				</div>
				<div className="flex flex-col gap-2">
					<div className="flex items-center justify-between">
						<Label htmlFor="password">Kata sandi</Label>
						<ForgotPassword />
					</div>
					<div className="relative">
						<Input
							id="password"
							name="password"
							type={showPassword ? "text" : "password"}
							autoComplete="current-password"
							placeholder="Masukkan kata sandi"
							className="pr-10"
							required
						/>
						<Button
							type="button"
							variant="ghost"
							size="icon-sm"
							className="absolute top-0.5 right-0.5"
							aria-pressed={showPassword}
							aria-label={
								showPassword ? "Sembunyikan kata sandi" : "Tampilkan kata sandi"
							}
							onClick={() => setShowPassword((on) => !on)}
						>
							{showPassword ? <EyeOffIcon /> : <EyeIcon />}
						</Button>
					</div>
				</div>
				<Label className="gap-2 font-normal">
					<Checkbox
						checked={remember}
						onCheckedChange={(checked) => setRemember(checked === true)}
					/>
					Ingat saya di perangkat ini
				</Label>
				{login.error ? (
					<p role="alert" className="text-sm text-destructive">
						{messageFor(login.error.messageCode)}
					</p>
				) : null}
				<Button type="submit" size="lg" disabled={login.isPending}>
					{login.isPending ? "Memproses…" : "Masuk"}
				</Button>
			</form>
			{supported ? (
				<div className="mt-4 flex flex-col gap-4">
					<div className="flex items-center gap-3 text-xs text-muted-foreground">
						<Separator className="flex-1" />
						atau masuk dengan
						<Separator className="flex-1" />
					</div>
					{passkeyError ? (
						<p role="alert" className="text-sm text-destructive">
							{passkeyError}
						</p>
					) : null}
					<Button
						type="button"
						variant="outline"
						size="lg"
						disabled={passkeyPending || login.isPending}
						onClick={() => void onPasskey()}
					>
						<KeyRoundIcon data-icon="inline-start" />
						Passkey
					</Button>
				</div>
			) : null}
		</AuthShell>
	);
}

// ADR 0007: there is no email reset. An Administrator issues a new link.
function ForgotPassword() {
	return (
		<Dialog>
			<DialogTrigger
				render={
					<Button
						type="button"
						variant="link"
						size="xs"
						className="h-auto p-0"
					/>
				}
			>
				Lupa kata sandi?
			</DialogTrigger>
			<DialogContent>
				<DialogHeader>
					<DialogTitle>Lupa kata sandi?</DialogTitle>
					<DialogDescription>
						Hubungi Administrator desa untuk mendapatkan tautan akses baru.
					</DialogDescription>
				</DialogHeader>
			</DialogContent>
		</Dialog>
	);
}
