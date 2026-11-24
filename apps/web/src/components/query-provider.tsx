import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { type ReactNode, useState } from "react";

// One client per mounted tree: on the server a module-level client would be
// shared between visitors.
export function QueryProvider({ children }: Readonly<{ children: ReactNode }>) {
	const [client] = useState(
		() =>
			new QueryClient({
				defaultOptions: { queries: { retry: false, staleTime: 0 } },
			}),
	);
	return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}
