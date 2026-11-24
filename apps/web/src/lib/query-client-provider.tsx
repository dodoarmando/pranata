import {
	MutationCache,
	QueryCache,
	QueryClient,
	QueryClientProvider,
} from "@tanstack/react-query";
import { useRouter } from "@tanstack/react-router";
import { type ReactNode, useRef, useState } from "react";
import { ApiError } from "@/api/fetcher";
import { setCsrfToken } from "@/lib/csrf";
import { expiryRedirect } from "@/lib/session-expiry";

// Admin-only: the public pages must never import this (it pulls in react-query).
// One client per mounted tree; an expired session anywhere clears cached admin
// data and the CSRF token, then returns to /masuk.
export function AppQueryProvider({
	children,
}: Readonly<{ children: ReactNode }>) {
	const router = useRouter();
	const routerRef = useRef(router);
	routerRef.current = router;

	const [client] = useState(() => {
		let handled = false;
		const onError = (error: unknown) => {
			if (!(error instanceof ApiError) || handled) return;
			const current = routerRef.current;
			const target = expiryRedirect(error.code, current.state.location.href);
			if (!target) return;
			handled = true;
			setCsrfToken(null);
			client.clear();
			void current.history.push(target);
			setTimeout(() => {
				handled = false;
			}, 1000);
		};
		const client: QueryClient = new QueryClient({
			queryCache: new QueryCache({ onError }),
			mutationCache: new MutationCache({ onError }),
			defaultOptions: { queries: { retry: false, staleTime: 0 } },
		});
		return client;
	});

	return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}
