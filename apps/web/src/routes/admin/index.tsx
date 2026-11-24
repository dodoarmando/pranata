import { createFileRoute } from "@tanstack/react-router";
import { AdminHome } from "@/admin/layout";

export const Route = createFileRoute("/admin/")({
	head: () => ({ meta: [{ title: "Beranda · Admin Pranata" }] }),
	component: Home,
});

function Home() {
	const { session } = Route.useRouteContext();
	return <AdminHome name={session.user.display_name} />;
}
