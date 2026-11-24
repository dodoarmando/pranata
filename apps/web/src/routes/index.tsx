import { createFileRoute } from "@tanstack/react-router";

export const Route = createFileRoute("/")({
	component: Home,
});

// Placeholder: Task 11 replaces this with the public village home page.
function Home() {
	return (
		<main className="p-8">
			<h1 className="text-2xl font-semibold">Pranata</h1>
		</main>
	);
}
