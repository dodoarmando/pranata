import { useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { KeyRoundIcon, PlusIcon } from "lucide-react";
import { type FormEvent, useState } from "react";
import { toast } from "sonner";
import { useSudo } from "@/admin/sudo";
import { ApiError } from "@/api/fetcher";
import type { PasskeySummary } from "@/api/generated/model";
import {
	getListPasskeysQueryKey,
	useDeletePasskey,
	useListPasskeys,
} from "@/api/generated/passkeys/passkeys";
import {
	getGetPublicVillageQueryKey,
	getPublicVillage,
} from "@/api/generated/public/public";
import { Button } from "@/components/ui/button";
import {
	Card,
	CardContent,
	CardDescription,
	CardHeader,
	CardTitle,
} from "@/components/ui/card";
import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogFooter,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import { Field, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { messageFor } from "@/lib/errors";
import { passkeysSupported, registerPasskey } from "@/lib/passkey";

export const Route = createFileRoute("/admin/akun")({
	head: () => ({ meta: [{ title: "Keamanan akun · Admin Pranata" }] }),
	component: AccountPage,
});

const FALLBACK_TIME_ZONE = "Asia/Jakarta";

function errorMessage(error: unknown): string {
	return messageFor(
		error instanceof ApiError ? error.messageCode : "passkey_failed",
	);
}

function formatDay(iso: string, timeZone: string): string {
	return new Intl.DateTimeFormat("id-ID", {
		dateStyle: "long",
		timeZone,
	}).format(new Date(iso));
}

function AccountPage() {
	const passkeys = useListPasskeys();
	// Not the generated hook, as on the audit log: keeps the query light.
	const village = useQuery({
		queryKey: getGetPublicVillageQueryKey(),
		queryFn: ({ signal }) => getPublicVillage({ signal }),
	});
	const timeZone = village.data?.time_zone ?? FALLBACK_TIME_ZONE;
	const [adding, setAdding] = useState(false);
	const [name, setName] = useState("");
	const [deleting, setDeleting] = useState<string | null>(null);
	const queryClient = useQueryClient();
	const sudo = useSudo();
	const remove = useDeletePasskey();
	const refresh = () =>
		queryClient.invalidateQueries({ queryKey: getListPasskeysQueryKey() });

	async function add(event: FormEvent) {
		event.preventDefault();
		const trimmed = name.trim();
		if (!trimmed) return;
		// Close first: Sudo Mode may open its own dialog on top.
		setAdding(false);
		try {
			const created = await sudo(() => registerPasskey(trimmed));
			if (created) {
				toast.success("Passkey ditambahkan.");
				setName("");
				await refresh();
			} else {
				setAdding(true);
			}
		} catch (error) {
			toast.error(errorMessage(error));
			setAdding(true);
		}
	}

	async function confirmDelete(passkey: PasskeySummary) {
		setDeleting(passkey.id);
		try {
			await sudo(() => remove.mutateAsync({ id: passkey.id }));
			toast.success("Passkey dihapus.");
			await refresh();
		} catch (error) {
			toast.error(errorMessage(error));
		} finally {
			setDeleting(null);
		}
	}

	return (
		<div className="mx-auto flex max-w-2xl flex-col gap-4">
			<h2 className="text-2xl font-semibold">Keamanan akun</h2>
			<Card>
				<CardHeader>
					<CardTitle>Passkey</CardTitle>
					<CardDescription>
						Masuk dengan sidik jari, wajah, atau kunci layar perangkat Anda.
					</CardDescription>
				</CardHeader>
				<CardContent className="flex flex-col gap-4">
					{passkeys.error ? (
						<p role="alert" className="text-destructive">
							{errorMessage(passkeys.error)}
						</p>
					) : !passkeys.data ? (
						<Skeleton className="h-16 w-full" />
					) : passkeys.data.length === 0 ? (
						<p className="text-sm text-muted-foreground">
							Belum ada passkey. Dengan passkey, Anda bisa masuk tanpa kata
							sandi, dan tetap bisa masuk jika lupa kata sandi.
						</p>
					) : (
						<ul className="flex flex-col divide-y rounded-lg border">
							{passkeys.data.map((passkey) => (
								<li
									key={passkey.id}
									className="flex items-center justify-between gap-3 p-3"
								>
									<div className="flex items-center gap-3">
										<KeyRoundIcon
											aria-hidden="true"
											className="size-4 text-muted-foreground"
										/>
										<div className="grid leading-tight">
											<span className="font-medium">{passkey.name}</span>
											<span className="text-xs text-muted-foreground">
												Dibuat {formatDay(passkey.created_at, timeZone)}
												{" · "}
												{passkey.last_used_at
													? `Terakhir dipakai ${formatDay(passkey.last_used_at, timeZone)}`
													: "Belum pernah dipakai"}
											</span>
										</div>
									</div>
									<Button
										type="button"
										variant="destructive"
										size="sm"
										aria-label={`Hapus passkey ${passkey.name}`}
										disabled={deleting === passkey.id}
										onClick={() => void confirmDelete(passkey)}
									>
										Hapus
									</Button>
								</li>
							))}
						</ul>
					)}
					<div>
						<Button
							type="button"
							disabled={!passkeysSupported()}
							onClick={() => setAdding(true)}
						>
							<PlusIcon data-icon="inline-start" />
							Tambah passkey
						</Button>
					</div>
				</CardContent>
			</Card>

			<Dialog open={adding} onOpenChange={setAdding}>
				<DialogContent>
					<form onSubmit={(e) => void add(e)} className="grid gap-4">
						<DialogHeader>
							<DialogTitle>Tambah passkey</DialogTitle>
							<DialogDescription>
								Beri nama agar Anda mengenali perangkatnya.
							</DialogDescription>
						</DialogHeader>
						<Field>
							<FieldLabel htmlFor="passkey-name">Nama passkey</FieldLabel>
							<Input
								id="passkey-name"
								placeholder="mis. HP Android saya"
								maxLength={50}
								autoFocus
								required
								value={name}
								onChange={(e) => setName(e.target.value)}
							/>
						</Field>
						<DialogFooter>
							<Button
								type="button"
								variant="outline"
								onClick={() => setAdding(false)}
							>
								Batal
							</Button>
							<Button type="submit" disabled={!name.trim()}>
								Lanjutkan
							</Button>
						</DialogFooter>
					</form>
				</DialogContent>
			</Dialog>
		</div>
	);
}
