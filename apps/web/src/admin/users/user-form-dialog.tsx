import { useQueryClient } from "@tanstack/react-query";
import { type FormEvent, useState } from "react";
import { toast } from "sonner";
import { useSudo } from "@/admin/sudo";
import { ApiError } from "@/api/fetcher";
import type {
	CreatedUser,
	CurrentUser,
	RoleDetail,
	UserSummary,
} from "@/api/generated/model";
import {
	getListUsersQueryKey,
	useCreateUser,
	useUpdateUser,
} from "@/api/generated/users/users";
import { Button } from "@/components/ui/button";
import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogFooter,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import { Field, FieldDescription, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import { messageFor } from "@/lib/errors";

export type UserFormTarget =
	| { kind: "create" }
	| { kind: "edit"; user: UserSummary };

// Only an Administrator may pick the Administrator Role, and nobody may hand
// out a Permission they lack. The API refuses both; the picker just does not
// offer them.
export function assignableRoles(
	roles: readonly RoleDetail[],
	me: CurrentUser,
): RoleDetail[] {
	if (me.role.is_administrator) return [...roles];
	return roles.filter(
		(role) =>
			!role.is_administrator &&
			role.permissions.every((permission) =>
				me.permissions.includes(permission),
			),
	);
}

export function UserFormDialog({
	target,
	roles,
	me,
	onClose,
	onCreated,
}: Readonly<{
	target: UserFormTarget | null;
	roles: readonly RoleDetail[];
	me: CurrentUser;
	onClose: () => void;
	onCreated: (created: CreatedUser) => void;
}>) {
	return (
		<Dialog
			open={target !== null}
			onOpenChange={(open) => {
				if (!open) onClose();
			}}
		>
			<DialogContent>
				{target ? (
					<UserForm
						target={target}
						roles={roles}
						me={me}
						onClose={onClose}
						onCreated={onCreated}
					/>
				) : null}
			</DialogContent>
		</Dialog>
	);
}

function UserForm({
	target,
	roles,
	me,
	onClose,
	onCreated,
}: Readonly<{
	target: UserFormTarget;
	roles: readonly RoleDetail[];
	me: CurrentUser;
	onClose: () => void;
	onCreated: (created: CreatedUser) => void;
}>) {
	const queryClient = useQueryClient();
	const sudo = useSudo();
	const createUser = useCreateUser();
	const updateUser = useUpdateUser();
	const editing = target.kind === "edit" ? target.user : null;
	const isSelf = editing?.id === me.id;
	const [displayName, setDisplayName] = useState(editing?.display_name ?? "");
	const [username, setUsername] = useState("");
	const [roleId, setRoleId] = useState<string | null>(editing?.role.id ?? null);
	const pending = createUser.isPending || updateUser.isPending;

	// Someone editing a User whose current Role is outside the picker still sees
	// it selected; it simply cannot be chosen again.
	const options = assignableRoles(roles, me);
	const items = (
		editing && !options.some((r) => r.id === editing.role.id)
			? [...options, { id: editing.role.id, name: editing.role.name }]
			: options
	).map((role) => ({ value: role.id, label: role.name }));

	async function submit(event: FormEvent) {
		event.preventDefault();
		if (!roleId) return;
		try {
			if (editing) {
				// The API answers `own_role` for any role_id sent for yourself,
				// even an unchanged one, so it is left out.
				await sudo(() =>
					updateUser.mutateAsync({
						id: editing.id,
						data: {
							display_name: displayName.trim(),
							...(isSelf || roleId === editing.role.id
								? {}
								: { role_id: roleId }),
						},
					}),
				);
				toast.success("Pengguna disimpan.");
				onClose();
			} else {
				const created = await sudo(() =>
					createUser.mutateAsync({
						data: {
							display_name: displayName.trim(),
							username: username.trim(),
							role_id: roleId,
						},
					}),
				);
				onClose();
				onCreated(created);
			}
			await queryClient.invalidateQueries({ queryKey: getListUsersQueryKey() });
		} catch (error) {
			toast.error(
				messageFor(
					error instanceof ApiError ? error.messageCode : "internal_error",
				),
			);
		}
	}

	return (
		<form onSubmit={submit} className="grid gap-4">
			<DialogHeader>
				<DialogTitle>
					{editing ? "Ubah pengguna" : "Tambah pengguna"}
				</DialogTitle>
				<DialogDescription>
					{editing
						? `@${editing.username}`
						: "Pengguna baru mendapat tautan akses untuk mengatur kata sandinya."}
				</DialogDescription>
			</DialogHeader>
			<Field>
				<FieldLabel htmlFor="user-display-name">Nama lengkap</FieldLabel>
				<Input
					id="user-display-name"
					required
					autoComplete="off"
					value={displayName}
					onChange={(e) => setDisplayName(e.target.value)}
				/>
			</Field>
			{editing ? null : (
				<Field>
					<FieldLabel htmlFor="user-username">Nama pengguna</FieldLabel>
					<Input
						id="user-username"
						required
						autoComplete="off"
						autoCapitalize="none"
						spellCheck={false}
						value={username}
						onChange={(e) => setUsername(e.target.value)}
					/>
					<FieldDescription>
						Tidak dapat diubah setelah dibuat.
					</FieldDescription>
				</Field>
			)}
			<Field>
				<FieldLabel htmlFor="user-role">Peran</FieldLabel>
				<Select
					items={items}
					value={roleId}
					disabled={isSelf}
					onValueChange={(value) => setRoleId(value)}
				>
					<SelectTrigger id="user-role" className="w-full">
						<SelectValue placeholder="Pilih peran" />
					</SelectTrigger>
					<SelectContent>
						{items.map((item) => (
							<SelectItem key={item.value} value={item.value}>
								{item.label}
							</SelectItem>
						))}
					</SelectContent>
				</Select>
				{isSelf ? (
					<FieldDescription>
						Anda tidak dapat mengubah peran Anda sendiri.
					</FieldDescription>
				) : null}
			</Field>
			<DialogFooter>
				<Button type="button" variant="outline" onClick={onClose}>
					Batal
				</Button>
				<Button
					type="submit"
					disabled={pending || !roleId || !displayName.trim()}
				>
					{pending ? "Menyimpan…" : "Simpan"}
				</Button>
			</DialogFooter>
		</form>
	);
}
