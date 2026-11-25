import { useRouteContext } from "@tanstack/react-router";
import type { ReactNode } from "react";
import type { Permission } from "@/api/generated/model";
import { Card, CardContent } from "@/components/ui/card";
import { can } from "@/lib/permissions";

// Wraps an admin screen. Without the Permission the screen is never mounted,
// so its data hooks do not run. No redirect: the URL stays where it is.
export function RequirePermission({
	permission,
	children,
}: Readonly<{ permission: Permission; children: ReactNode }>) {
	const { session } = useRouteContext({ from: "/admin" });
	if (!can(session, permission)) {
		return (
			<Card className="mx-auto max-w-xl">
				<CardContent role="alert">
					Anda tidak memiliki hak akses untuk halaman ini.
				</CardContent>
			</Card>
		);
	}
	return children;
}
