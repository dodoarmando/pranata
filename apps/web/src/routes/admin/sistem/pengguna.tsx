import { useQueryClient } from "@tanstack/react-query";
import { createFileRoute, useRouteContext } from "@tanstack/react-router";
import { MoreHorizontalIcon, PlusIcon } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
import { RequirePermission } from "@/admin/require-permission";
import { initials } from "@/admin/session";
import { useSudo } from "@/admin/sudo";
import {
	AccessLinkDialog,
	type IssuedLink,
} from "@/admin/users/access-link-dialog";
import {
	UserFormDialog,
	type UserFormTarget,
} from "@/admin/users/user-form-dialog";
import { ApiError } from "@/api/fetcher";
import type {
	CurrentUser,
	RoleDetail,
	UserSummary,
} from "@/api/generated/model";
import { useListRoles } from "@/api/generated/roles/roles";
import {
	getListUsersQueryKey,
	useDeactivateUser,
	useIssueAccessLink,
	useListUsers,
	useReactivateUser,
	useRemoveUserPasskeys,
} from "@/api/generated/users/users";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogFooter,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import {
	DropdownMenu,
	DropdownMenuContent,
	DropdownMenuItem,
	DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Skeleton } from "@/components/ui/skeleton";
import {
	Table,
	TableBody,
	TableCell,
	TableHead,
	TableHeader,
	TableRow,
} from "@/components/ui/table";
import { messageFor } from "@/lib/errors";

export const Route = createFileRoute("/admin/sistem/pengguna")({
	head: () => ({ meta: [{ title: "Pengguna · Admin Pranata" }] }),
	component: () => (
		<RequirePermission permission="user.manage">
			<UsersPage />
		</RequirePermission>
	),
});

function errorMessage(error: unknown): string {
	return messageFor(
		error instanceof ApiError ? error.messageCode : "internal_error",
	);
}

// The API refuses writes on a User who is an Administrator (unless you are
// one) or whose Role holds a Permission you lack (`stronger_user`). The screen
// just does not offer them. Until the Roles load nothing can be judged.
function canManage(
	me: CurrentUser,
	user: UserSummary,
	roles: readonly RoleDetail[],
): boolean {
	if (me.role.is_administrator) return true;
	if (user.role.is_administrator) return false;
	const role = roles.find((r) => r.id === user.role.id);
	return role?.permissions.every((p) => me.permissions.includes(p)) ?? false;
}

function UsersPage() {
	const { session } = useRouteContext({ from: "/admin" });
	const me = session.user;
	const users = useListUsers();
	const roles = useListRoles();
	const [form, setForm] = useState<UserFormTarget | null>(null);
	const [link, setLink] = useState<IssuedLink | null>(null);
	const [deactivating, setDeactivating] = useState<UserSummary | null>(null);
	const [clearing, setClearing] = useState<UserSummary | null>(null);

	const error = users.error ?? roles.error;
	if (error) {
		return (
			<p role="alert" className="text-destructive">
				{errorMessage(error)}
			</p>
		);
	}
	if (!users.data || !roles.data) {
		return (
			<div className="mx-auto flex max-w-4xl flex-col gap-4">
				<Skeleton className="h-8 w-48" />
				<Skeleton className="h-64 w-full" />
			</div>
		);
	}
	const roleList = roles.data;

	return (
		<div className="mx-auto flex max-w-4xl flex-col gap-4">
			<div className="flex flex-wrap items-start justify-between gap-3">
				<div className="grid gap-1">
					<h2 className="text-2xl font-semibold">Pengguna</h2>
					<p className="text-sm text-muted-foreground">
						Pengguna yang berhenti dinonaktifkan; nama penggunanya tidak dipakai
						lagi.
					</p>
				</div>
				<Button type="button" onClick={() => setForm({ kind: "create" })}>
					<PlusIcon data-icon="inline-start" />
					Tambah pengguna
				</Button>
			</div>

			<Table>
				<TableHeader>
					<TableRow>
						<TableHead>Pengguna</TableHead>
						<TableHead>Peran</TableHead>
						<TableHead>Status</TableHead>
						<TableHead>
							<span className="sr-only">Aksi</span>
						</TableHead>
					</TableRow>
				</TableHeader>
				<TableBody>
					{users.data.map((user) => (
						<UserRow
							key={user.id}
							user={user}
							isSelf={user.id === me.id}
							manageable={canManage(me, user, roleList)}
							onEdit={() => setForm({ kind: "edit", user })}
							onLink={setLink}
							onDeactivate={() => setDeactivating(user)}
							onClearPasskeys={() => setClearing(user)}
						/>
					))}
				</TableBody>
			</Table>

			<UserFormDialog
				target={form}
				roles={roleList}
				me={me}
				onClose={() => setForm(null)}
				onCreated={(created) =>
					setLink({
						displayName: created.user.display_name,
						url: created.access_link.url,
					})
				}
			/>
			<AccessLinkDialog link={link} onClose={() => setLink(null)} />
			<DeactivateDialog
				user={deactivating}
				onClose={() => setDeactivating(null)}
			/>
			<RemovePasskeysDialog user={clearing} onClose={() => setClearing(null)} />
		</div>
	);
}

function UserRow({
	user,
	isSelf,
	manageable,
	onEdit,
	onLink,
	onDeactivate,
	onClearPasskeys,
}: Readonly<{
	user: UserSummary;
	isSelf: boolean;
	manageable: boolean;
	onEdit: () => void;
	onLink: (link: IssuedLink) => void;
	onDeactivate: () => void;
	onClearPasskeys: () => void;
}>) {
	const queryClient = useQueryClient();
	const sudo = useSudo();
	const issue = useIssueAccessLink();
	const reactivate = useReactivateUser();

	async function issueLink() {
		try {
			const issued = await sudo(() => issue.mutateAsync({ id: user.id }));
			onLink({ displayName: user.display_name, url: issued.url });
		} catch (error) {
			toast.error(errorMessage(error));
		}
	}

	async function activate() {
		try {
			await sudo(() => reactivate.mutateAsync({ id: user.id }));
			toast.success("Pengguna diaktifkan kembali.");
			await queryClient.invalidateQueries({ queryKey: getListUsersQueryKey() });
		} catch (error) {
			toast.error(errorMessage(error));
		}
	}

	return (
		<TableRow>
			<TableCell>
				<div className="flex items-center gap-3">
					<span
						aria-hidden="true"
						className="grid size-8 shrink-0 place-items-center rounded-lg bg-muted text-xs font-medium"
					>
						{initials(user.display_name)}
					</span>
					<span className="grid leading-tight">
						<span className="font-medium">{user.display_name}</span>
						<span className="text-xs text-muted-foreground">
							@{user.username}
						</span>
					</span>
				</div>
			</TableCell>
			<TableCell>
				<Badge variant={user.role.is_administrator ? "default" : "secondary"}>
					{user.role.name}
				</Badge>
			</TableCell>
			<TableCell>
				<Badge variant={user.is_active ? "outline" : "secondary"}>
					{user.is_active ? "Aktif" : "Nonaktif"}
				</Badge>
			</TableCell>
			<TableCell className="text-right">
				{manageable ? (
					<DropdownMenu>
						<DropdownMenuTrigger
							render={
								<Button
									type="button"
									variant="ghost"
									size="icon"
									aria-label={`Menu untuk ${user.display_name}`}
								/>
							}
						>
							<MoreHorizontalIcon />
						</DropdownMenuTrigger>
						<DropdownMenuContent align="end">
							<DropdownMenuItem onClick={onEdit}>Ubah</DropdownMenuItem>
							{user.is_active ? (
								<DropdownMenuItem onClick={() => void issueLink()}>
									Buat tautan akses
								</DropdownMenuItem>
							) : null}
							{user.passkey_count > 0 ? (
								<DropdownMenuItem
									variant="destructive"
									onClick={onClearPasskeys}
								>
									Hapus semua passkey
								</DropdownMenuItem>
							) : null}
							{user.is_active ? (
								isSelf ? null : (
									<DropdownMenuItem
										variant="destructive"
										onClick={onDeactivate}
									>
										Nonaktifkan
									</DropdownMenuItem>
								)
							) : (
								<DropdownMenuItem onClick={() => void activate()}>
									Aktifkan kembali
								</DropdownMenuItem>
							)}
						</DropdownMenuContent>
					</DropdownMenu>
				) : null}
			</TableCell>
		</TableRow>
	);
}

function DeactivateDialog({
	user,
	onClose,
}: Readonly<{ user: UserSummary | null; onClose: () => void }>) {
	const queryClient = useQueryClient();
	const sudo = useSudo();
	const deactivate = useDeactivateUser();

	async function confirm(target: UserSummary) {
		try {
			await sudo(() => deactivate.mutateAsync({ id: target.id }));
			toast.success("Pengguna dinonaktifkan.");
			onClose();
			await queryClient.invalidateQueries({ queryKey: getListUsersQueryKey() });
		} catch (error) {
			toast.error(errorMessage(error));
		}
	}

	return (
		<Dialog
			open={user !== null}
			onOpenChange={(open) => {
				if (!open) onClose();
			}}
		>
			<DialogContent>
				{user ? (
					<>
						<DialogHeader>
							<DialogTitle>Nonaktifkan pengguna</DialogTitle>
							<DialogDescription>
								Nonaktifkan {user.display_name}? Sesi masuknya berakhir dan
								tautan aksesnya tidak berlaku lagi.
							</DialogDescription>
						</DialogHeader>
						<DialogFooter>
							<Button type="button" variant="outline" onClick={onClose}>
								Batal
							</Button>
							<Button
								type="button"
								variant="destructive"
								disabled={deactivate.isPending}
								onClick={() => void confirm(user)}
							>
								Nonaktifkan
							</Button>
						</DialogFooter>
					</>
				) : null}
			</DialogContent>
		</Dialog>
	);
}

function RemovePasskeysDialog({
	user,
	onClose,
}: Readonly<{ user: UserSummary | null; onClose: () => void }>) {
	const queryClient = useQueryClient();
	const sudo = useSudo();
	const removePasskeys = useRemoveUserPasskeys();

	async function confirm(target: UserSummary) {
		try {
			await sudo(() => removePasskeys.mutateAsync({ id: target.id }));
			toast.success("Passkey dihapus.");
			onClose();
			await queryClient.invalidateQueries({ queryKey: getListUsersQueryKey() });
		} catch (error) {
			toast.error(errorMessage(error));
		}
	}

	return (
		<Dialog
			open={user !== null}
			onOpenChange={(open) => {
				if (!open) onClose();
			}}
		>
			<DialogContent>
				{user ? (
					<>
						<DialogHeader>
							<DialogTitle>Hapus semua passkey</DialogTitle>
							<DialogDescription>
								Hapus semua passkey milik {user.display_name}? Ia harus masuk
								dengan kata sandi atau tautan akses baru.
							</DialogDescription>
						</DialogHeader>
						<DialogFooter>
							<Button type="button" variant="outline" onClick={onClose}>
								Batal
							</Button>
							<Button
								type="button"
								variant="destructive"
								disabled={removePasskeys.isPending}
								onClick={() => void confirm(user)}
							>
								Hapus semua
							</Button>
						</DialogFooter>
					</>
				) : null}
			</DialogContent>
		</Dialog>
	);
}
