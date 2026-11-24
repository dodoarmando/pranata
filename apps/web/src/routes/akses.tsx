import { createFileRoute, useRouter } from "@tanstack/react-router";
import { type FormEvent, useEffect, useState } from "react";
import { useSetPasswordWithAccessLink } from "@/api/generated/auth/auth";
import { AuthShell } from "@/components/auth-shell";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { parseAccessToken } from "@/lib/access-token";
import { messageFor } from "@/lib/errors";
import { AppQueryProvider } from "@/lib/query-client-provider";

export const Route = createFileRoute("/akses")({
	head: () => ({
		meta: [
			{ title: "Atur kata sandi" },
			// Keep the page out of search results and referrers.
			{ name: "robots", content: "noindex" },
			{ name: "referrer", content: "no-referrer" },
		],
	}),
	component: AccessPage,
});

const MIN = 10;
const MAX = 128;
const EXPIRED = messageFor("access_link_expired");

type TokenState =
	| { status: "reading" }
	| { status: "missing" }
	| { status: "ready"; token: string };

function AccessPage() {
	return (
		<AppQueryProvider>
			<Access />
		</AppQueryProvider>
	);
}

function Access() {
	const router = useRouter();
	const [state, setState] = useState<TokenState>({ status: "reading" });
	const [clientError, setClientError] = useState<string | null>(null);

	// The token is in the fragment and must never reach a server, log or
	// history entry. Read it once on the client, drop it from the URL, and keep
	// it only in component state.
	useEffect(() => {
		const token = parseAccessToken(window.location.hash);
		window.history.replaceState(null, "", window.location.pathname);
		setState(token ? { status: "ready", token } : { status: "missing" });
	}, []);

	const redeem = useSetPasswordWithAccessLink({
		mutation: { onSuccess: () => void router.navigate({ to: "/masuk" }) },
	});

	function onSubmit(event: FormEvent<HTMLFormElement>) {
		event.preventDefault();
		if (state.status !== "ready") return;
		const form = new FormData(event.currentTarget);
		const password = String(form.get("password") ?? "");
		const confirmation = String(form.get("confirmation") ?? "");
		// The server counts characters, not UTF-16 units, so emoji count once.
		const length = [...password].length;
		if (length < MIN || length > MAX) {
			setClientError(messageFor("password_length"));
			return;
		}
		if (password !== confirmation) {
			setClientError("Konfirmasi kata sandi tidak sama.");
			return;
		}
		setClientError(null);
		redeem.mutate({ data: { token: state.token, password } });
	}

	const apiError = redeem.error ? messageFor(redeem.error.messageCode) : null;
	const error = clientError ?? apiError;

	return (
		<AuthShell
			title="Atur kata sandi"
			subtitle="Buat kata sandi untuk masuk ke admin desa."
		>
			{state.status === "missing" ? (
				<p role="alert" className="text-center text-sm">
					{EXPIRED}
				</p>
			) : (
				<form onSubmit={onSubmit} className="flex flex-col gap-4" noValidate>
					<div className="flex flex-col gap-2">
						<Label htmlFor="password">Kata sandi baru</Label>
						<Input
							id="password"
							name="password"
							type="password"
							autoComplete="new-password"
							minLength={MIN}
							maxLength={MAX}
							required
						/>
						<p className="text-xs text-muted-foreground">10–128 karakter.</p>
					</div>
					<div className="flex flex-col gap-2">
						<Label htmlFor="confirmation">Ulangi kata sandi</Label>
						<Input
							id="confirmation"
							name="confirmation"
							type="password"
							autoComplete="new-password"
							maxLength={MAX}
							required
						/>
					</div>
					{error ? (
						<p role="alert" className="text-sm text-destructive">
							{error}
						</p>
					) : null}
					<Button
						type="submit"
						size="lg"
						disabled={state.status !== "ready" || redeem.isPending}
					>
						{redeem.isPending ? "Menyimpan…" : "Simpan kata sandi"}
					</Button>
				</form>
			)}
		</AuthShell>
	);
}
