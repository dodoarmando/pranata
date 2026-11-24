import { createFileRoute, Outlet, redirect } from "@tanstack/react-router";
import { AdminLayout } from "@/admin/layout";
import { loadSession } from "@/admin/session";

// Admin is a client-rendered app: the session cookie is HttpOnly and the CSRF
// token comes from /api/auth/me, so nothing here can be rendered on the server.
export const Route = createFileRoute("/admin")({
	ssr: false,
	beforeLoad: async ({ location }) => {
		const session = await loadSession();
		if (!session) {
			throw redirect({ to: "/masuk", search: { ke: location.href } });
		}
		return { session };
	},
	component: AdminShell,
});

function AdminShell() {
	const { session } = Route.useRouteContext();
	return (
		<AdminLayout user={session.user}>
			<Outlet />
		</AdminLayout>
	);
}
