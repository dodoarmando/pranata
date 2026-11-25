import { describe, expect, it } from "@rstest/core";
import { MutationObserver, QueryClient } from "@tanstack/react-query";
import { SECRET_RESULT } from "./secret-result";

function settle() {
	return new Promise((resolve) => setTimeout(resolve, 10));
}

async function cachedAfterReset(options: { gcTime?: number }) {
	const client = new QueryClient();
	const observer = new MutationObserver(client, {
		mutationFn: async () => ({ url: "http://localhost:3000/akses#secret" }),
		...options,
	});
	await observer.mutate();
	observer.reset();
	await settle();
	return client.getMutationCache().getAll().length;
}

describe("SECRET_RESULT", () => {
	it("drops a finished mutation, URL included, once it is reset", async () => {
		expect(await cachedAfterReset(SECRET_RESULT.mutation)).toBe(0);
	});

	it("is needed: by default the result stays in the cache", async () => {
		expect(await cachedAfterReset({})).toBe(1);
	});
});
