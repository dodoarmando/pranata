import { useQueryClient } from "@tanstack/react-query";
import { createFileRoute, useRouteContext } from "@tanstack/react-router";
import { LockIcon, PlusIcon, TrashIcon } from "lucide-react";
import { type FormEvent, useState } from "react";
import { toast } from "sonner";
import { RequirePermission } from "@/admin/require-permission";
import { useSudo } from "@/admin/sudo";
import { ApiError } from "@/api/fetcher";
import type {
	CurrentUser,
	Permission,
	RoleDetail,
} from "@/api/generated/model";
import {
	getListRolesQueryKey,
	useCreateRole,
	useDeleteRole,
	useListRoles,
	useUpdateRole,
} from "@/api/generated/roles/roles";
import { Button } from "@/components/ui/button";
import {
	Card,
	CardAction,
	CardContent,
	CardDescription,
	CardHeader,
	CardTitle,
} from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogFooter,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import {
	Field,
	FieldDescription,
	FieldLabel,
	FieldLegend,
	FieldSet,
} from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Skeleton } from "@/components/ui/skeleton";
import { messageFor } from "@/lib/errors";
import { PERMISSION_GROUPS, PERMISSION_LABELS } from "@/lib/permissions";
import { cn } from "@/lib/utils";

export const Route = createFileRoute("/admin/sistem/peran")({
	head: () => ({ meta: [{ title: "Peran · Admin Pranata" }] }),
	component: () => (
		<RequirePermission permission="role.manage">
			<RolesPage />
		</RequirePermission>
	),
});

const NEW_ROLE = "new";

function errorMessage(error: unknown): string {
	return messageFor(
		error instanceof ApiError ? error.messageCode : "internal_error",
	);
}

// The API refuses edits to the Administrator Role, to the Role you hold, and
// to a Role that grants a Permission you lack (unless you are an
// Administrator). The screen just does not offer them.
function isReadOnly(me: CurrentUser, role: RoleDetail): boolean {
	if (role.is_administrator) return true;
	if (me.role.is_administrator) return false;
	return (
		role.id === me.role.id ||
		!role.permissions.every((p) => me.permissions.includes(p))
	);
}

function RolesPage() {
	const { session } = useRouteContext({ from: "/admin" });
	const me = session.user;
	const roles = useListRoles();
	const [selected, setSelected] = useState<string | null>(null);

	if (roles.error) {
		return (
			<p role="alert" className="text-destructive">
				{errorMessage(roles.error)}
			</p>
		);
	}
	if (!roles.data) {
		return (
			<div className="mx-auto flex max-w-5xl flex-col gap-4">
				<Skeleton className="h-8 w-48" />
				<Skeleton className="h-64 w-full" />
			</div>
		);
	}
	const roleList = roles.data;
	const current =
		selected === NEW_ROLE
			? null
			: (roleList.find((r) => r.id === selected) ?? roleList[0] ?? null);
	const currentKey = selected === NEW_ROLE ? NEW_ROLE : current?.id;

	return (
		<div className="mx-auto flex max-w-5xl flex-col gap-4">
			<div className="flex flex-wrap items-start justify-between gap-3">
				<div className="grid gap-1">
					<h2 className="text-2xl font-semibold">Peran</h2>
					<p className="text-sm text-muted-foreground">
						Daftar hak akses tetap dari sistem; administrator hanya memilih hak
						akses tiap peran.
					</p>
				</div>
				<Button type="button" onClick={() => setSelected(NEW_ROLE)}>
					<PlusIcon data-icon="inline-start" />
					Tambah peran
				</Button>
			</div>

			<div className="grid items-start gap-4 md:grid-cols-[300px_1fr]">
				<Card size="sm">
					<CardContent className="grid gap-1">
						{roleList.map((role) => (
							<Button
								key={role.id}
								type="button"
								variant="ghost"
								aria-current={role.id === currentKey}
								className="h-auto justify-start gap-2 py-2 text-left aria-[current=true]:ring-1 aria-[current=true]:ring-foreground/30"
								onClick={() => setSelected(role.id)}
							>
								<span className="grid flex-1 leading-tight">
									<span className="font-medium">{role.name}</span>
									<span className="text-xs font-normal text-foreground/70">
										{role.is_administrator
											? "Semua hak akses, tidak dapat diubah"
											: `${role.user_count} pengguna · ${role.permissions.length} hak akses`}
									</span>
								</span>
								{role.is_administrator ? (
									<LockIcon
										aria-hidden="true"
										className="text-muted-foreground"
									/>
								) : null}
							</Button>
						))}
					</CardContent>
				</Card>

				<RoleEditor
					key={currentKey}
					me={me}
					role={current}
					onSaved={(role) => setSelected(role.id)}
					onDeleted={() => setSelected(null)}
				/>
			</div>
		</div>
	);
}

function RoleEditor({
	me,
	role,
	onSaved,
	onDeleted,
}: Readonly<{
	me: CurrentUser;
	role: RoleDetail | null;
	onSaved: (role: RoleDetail) => void;
	onDeleted: () => void;
}>) {
	const queryClient = useQueryClient();
	const sudo = useSudo();
	const createRole = useCreateRole();
	const updateRole = useUpdateRole();
	const deleteRole = useDeleteRole();
	const [name, setName] = useState(role?.name ?? "");
	const [granted, setGranted] = useState<ReadonlySet<Permission>>(
		new Set(role?.permissions ?? []),
	);
	const [confirmDelete, setConfirmDelete] = useState(false);
	const readOnly = role ? isReadOnly(me, role) : false;
	const pending = createRole.isPending || updateRole.isPending;

	function toggle(permission: Permission, on: boolean) {
		const next = new Set(granted);
		if (on) next.add(permission);
		else next.delete(permission);
		setGranted(next);
	}

	async function save(event: FormEvent) {
		event.preventDefault();
		const data = {
			name: name.trim(),
			permissions: PERMISSION_GROUPS.flatMap((g) => g.permissions).filter((p) =>
				granted.has(p),
			),
		};
		try {
			const saved = await sudo(() =>
				role
					? updateRole.mutateAsync({ id: role.id, data })
					: createRole.mutateAsync({ data }),
			);
			toast.success("Peran disimpan.");
			await queryClient.invalidateQueries({ queryKey: getListRolesQueryKey() });
			onSaved(saved);
		} catch (error) {
			toast.error(errorMessage(error));
		}
	}

	async function remove(target: RoleDetail) {
		try {
			await sudo(() => deleteRole.mutateAsync({ id: target.id }));
			toast.success("Peran dihapus.");
			setConfirmDelete(false);
			await queryClient.invalidateQueries({ queryKey: getListRolesQueryKey() });
			onDeleted();
		} catch (error) {
			setConfirmDelete(false);
			toast.error(errorMessage(error));
		}
	}

	return (
		<Card>
			<form onSubmit={save} className="grid gap-4">
				<CardHeader>
					<CardTitle>{role ? role.name : "Peran baru"}</CardTitle>
					<CardDescription>
						{role
							? `Dipakai ${role.user_count} pengguna.`
							: "Pilih hak akses yang diberikan peran ini."}
					</CardDescription>
					{readOnly ? null : (
						<CardAction className="flex gap-2">
							{role ? (
								<Button
									type="button"
									variant="ghost"
									size="sm"
									onClick={() => setConfirmDelete(true)}
								>
									<TrashIcon data-icon="inline-start" />
									Hapus
								</Button>
							) : null}
							<Button
								type="submit"
								size="sm"
								disabled={pending || !name.trim()}
							>
								{pending ? "Menyimpan…" : "Simpan"}
							</Button>
						</CardAction>
					)}
				</CardHeader>
				<CardContent className="grid gap-6">
					<Field className="max-w-xs">
						<FieldLabel htmlFor="role-name">Nama peran</FieldLabel>
						<Input
							id="role-name"
							required
							autoComplete="off"
							disabled={readOnly}
							value={name}
							onChange={(e) => setName(e.target.value)}
						/>
						{readOnly && !role?.is_administrator ? (
							<FieldDescription>
								Anda tidak dapat mengubah peran ini.
							</FieldDescription>
						) : null}
					</Field>
					<div className="grid gap-6 [grid-template-columns:repeat(auto-fit,minmax(240px,1fr))]">
						{PERMISSION_GROUPS.map((group) => (
							<FieldSet key={group.label}>
								<FieldLegend variant="label">{group.label}</FieldLegend>
								{group.permissions.map((permission) => {
									const lacking =
										!me.role.is_administrator &&
										!me.permissions.includes(permission);
									return (
										<Label
											key={permission}
											className={cn("items-start", lacking && "opacity-60")}
										>
											<Checkbox
												checked={
													role?.is_administrator || granted.has(permission)
												}
												disabled={readOnly || lacking}
												onCheckedChange={(on) => toggle(permission, on)}
											/>
											<span className="grid gap-0.5 leading-tight">
												<span>{PERMISSION_LABELS[permission]}</span>
												<span className="text-xs font-normal text-foreground/70">
													{permission}
												</span>
											</span>
										</Label>
									);
								})}
							</FieldSet>
						))}
					</div>
				</CardContent>
			</form>
			<Dialog open={confirmDelete} onOpenChange={setConfirmDelete}>
				<DialogContent>
					<DialogHeader>
						<DialogTitle>Hapus peran</DialogTitle>
						<DialogDescription>
							Hapus peran {role?.name}? Peran yang sudah dihapus tidak dapat
							dikembalikan.
						</DialogDescription>
					</DialogHeader>
					<DialogFooter>
						<Button
							type="button"
							variant="outline"
							onClick={() => setConfirmDelete(false)}
						>
							Batal
						</Button>
						<Button
							type="button"
							variant="destructive"
							disabled={deleteRole.isPending}
							onClick={() => role && void remove(role)}
						>
							Hapus
						</Button>
					</DialogFooter>
				</DialogContent>
			</Dialog>
		</Card>
	);
}
