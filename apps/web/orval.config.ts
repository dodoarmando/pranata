import { defineConfig } from "orval";

// Generated output lives in src/api/generated (clean-and-rewrite on every run).
// The hand-written fetch mutator sits one level up, outside that overwrite path.
export default defineConfig({
	pranata: {
		input: "../api/openapi.json",
		output: {
			client: "react-query",
			httpClient: "fetch",
			// Operation paths in the contract already start with /api, so the
			// generated URLs are same-origin relative and need no extra base URL.
			baseUrl: "",
			mode: "tags-split",
			target: "src/api/generated",
			schemas: "src/api/generated/model",
			clean: true,
			override: {
				mutator: { path: "src/api/fetcher.ts", name: "apiFetch" },
				fetch: { includeHttpResponseReturnType: false },
			},
		},
	},
});
