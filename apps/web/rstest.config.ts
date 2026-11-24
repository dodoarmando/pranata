import { defineConfig } from "@rstest/core";

export default defineConfig({
	resolve: { alias: { "@": "./src" } },
	// Playwright owns e2e/ (`just e2e`).
	exclude: ["**/node_modules/**", "**/dist/**", "e2e/**"],
});
