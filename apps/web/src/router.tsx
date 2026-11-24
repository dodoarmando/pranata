import { createRouter } from "@tanstack/react-router";
import { NONCE_PLACEHOLDER } from "@/lib/nonce";
import { routeTree } from "./routeTree.gen";

export function getRouter() {
	return createRouter({
		routeTree,
		scrollRestoration: true,
		// The server entry swaps this token for a per-response nonce (see server/handler.ts).
		ssr:
			typeof window === "undefined" ? { nonce: NONCE_PLACEHOLDER } : undefined,
	});
}

declare module "@tanstack/react-router" {
	interface Register {
		router: ReturnType<typeof getRouter>;
	}
}
