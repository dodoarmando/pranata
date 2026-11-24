import { useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate, useRouterState } from "@tanstack/react-router";
import { ChevronsUpDownIcon, HouseIcon, LogOutIcon } from "lucide-react";
import { type ReactNode, useEffect } from "react";
import { toast } from "sonner";
import { visibleNav } from "@/admin/nav";
import { initials } from "@/admin/session";
import { useLogout } from "@/api/generated/auth/auth";
import type { CurrentUser } from "@/api/generated/model";
import { Button } from "@/components/ui/button";
import {
	DropdownMenu,
	DropdownMenuContent,
	DropdownMenuItem,
	DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Separator } from "@/components/ui/separator";
import {
	Sidebar,
	SidebarContent,
	SidebarFooter,
	SidebarGroup,
	SidebarGroupContent,
	SidebarGroupLabel,
	SidebarHeader,
	SidebarInset,
	SidebarMenu,
	SidebarMenuButton,
	SidebarMenuItem,
	SidebarProvider,
	SidebarTrigger,
} from "@/components/ui/sidebar";
import { Toaster } from "@/components/ui/sonner";
import { setCsrfToken } from "@/lib/csrf";
import { messageFor } from "@/lib/errors";

const GROUPS = visibleNav();

function pageTitle(pathname: string): string {
	for (const group of GROUPS) {
		for (const item of group.items) {
			if (pathname.startsWith(item.to)) return item.label;
		}
	}
	return "Beranda";
}

function UserMenu({ user }: Readonly<{ user: CurrentUser }>) {
	const navigate = useNavigate();
	const queryClient = useQueryClient();
	const logout = useLogout({
		mutation: {
			onSuccess: () => {
				setCsrfToken(null);
				queryClient.clear();
				void navigate({ to: "/masuk", search: {} });
			},
			onError: (error) => toast.error(messageFor(error.code)),
		},
	});

	return (
		<DropdownMenu>
			<DropdownMenuTrigger
				render={
					<SidebarMenuButton
						size="lg"
						className="data-popup-open:bg-sidebar-accent"
					/>
				}
			>
				<span
					aria-hidden="true"
					className="grid size-8 shrink-0 place-items-center rounded-lg bg-muted text-xs font-medium"
				>
					{initials(user.display_name)}
				</span>
				<span className="grid flex-1 text-left leading-tight">
					<span className="truncate text-sm font-medium">
						{user.display_name}
					</span>
					<span className="truncate text-xs text-muted-foreground">
						{user.username}
					</span>
				</span>
				<ChevronsUpDownIcon className="ml-auto size-4 text-muted-foreground" />
			</DropdownMenuTrigger>
			<DropdownMenuContent side="top" align="start" className="min-w-56">
				<DropdownMenuItem
					disabled={logout.isPending}
					onClick={() => logout.mutate()}
				>
					<LogOutIcon />
					Keluar
				</DropdownMenuItem>
			</DropdownMenuContent>
		</DropdownMenu>
	);
}

export function AdminLayout({
	user,
	children,
}: Readonly<{ user: CurrentUser; children: ReactNode }>) {
	const pathname = useRouterState({ select: (s) => s.location.pathname });

	// Dialogs, sheets and menus portal to <body>, so the neutral admin tokens
	// have to live there rather than on a wrapper.
	useEffect(() => {
		document.body.classList.add("admin");
		return () => document.body.classList.remove("admin");
	}, []);

	const title = pageTitle(pathname);

	return (
		<SidebarProvider>
			<Sidebar>
				<SidebarHeader>
					<Link
						to="/admin"
						className="flex items-center gap-2 rounded-lg p-2 outline-none focus-visible:ring-2 focus-visible:ring-ring"
					>
						<span
							aria-hidden="true"
							className="grid size-8 place-items-center rounded-lg bg-primary text-sm font-semibold text-primary-foreground"
						>
							P
						</span>
						<span className="grid leading-tight">
							<span className="text-sm font-semibold">Pranata</span>
							<span className="text-xs text-muted-foreground">Admin desa</span>
						</span>
					</Link>
				</SidebarHeader>
				<SidebarContent>
					<SidebarGroup>
						<SidebarGroupContent>
							<SidebarMenu>
								<SidebarMenuItem>
									<SidebarMenuButton
										render={
											<Link to="/admin" activeOptions={{ exact: true }} />
										}
										isActive={pathname === "/admin"}
									>
										<HouseIcon />
										Beranda
									</SidebarMenuButton>
								</SidebarMenuItem>
							</SidebarMenu>
						</SidebarGroupContent>
					</SidebarGroup>
					{GROUPS.map((group) => (
						<SidebarGroup key={group.label}>
							<SidebarGroupLabel>{group.label}</SidebarGroupLabel>
							<SidebarGroupContent>
								<SidebarMenu>
									{group.items.map((item) => (
										<SidebarMenuItem key={item.to}>
											<SidebarMenuButton
												render={<Link to={item.to} />}
												isActive={pathname.startsWith(item.to)}
											>
												<group.icon />
												{item.label}
											</SidebarMenuButton>
										</SidebarMenuItem>
									))}
								</SidebarMenu>
							</SidebarGroupContent>
						</SidebarGroup>
					))}
				</SidebarContent>
				<SidebarFooter>
					<SidebarMenu>
						<SidebarMenuItem>
							<UserMenu user={user} />
						</SidebarMenuItem>
					</SidebarMenu>
				</SidebarFooter>
			</Sidebar>
			<SidebarInset>
				<header className="flex h-14 items-center gap-2 border-b px-4">
					<SidebarTrigger aria-label="Buka atau tutup menu" />
					<Separator orientation="vertical" className="h-5" />
					<h1 className="text-sm font-medium">{title}</h1>
				</header>
				<div id="konten" className="flex-1 p-4 md:p-6">
					{children}
				</div>
			</SidebarInset>
			<Toaster position="top-center" />
		</SidebarProvider>
	);
}

export function AdminHome({ name }: Readonly<{ name: string }>) {
	return (
		<div className="mx-auto flex max-w-2xl flex-col gap-4">
			<h2 className="text-2xl font-semibold">Selamat datang, {name}.</h2>
			<p className="text-muted-foreground">
				Mulai dari profil desa untuk memperbarui gambaran umum dan foto hero
				yang tampil di beranda situs.
			</p>
			<div>
				<Button render={<Link to="/admin/desa/profil" />}>
					Buka profil desa
				</Button>
			</div>
		</div>
	);
}
